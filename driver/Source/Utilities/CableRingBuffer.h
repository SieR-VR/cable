/*++

Module Name:

    CableRingBuffer.h

Abstract:

    Declaration of the CableRingBuffer class.
    
    This class manages a kernel-allocated shared memory region containing
    a CABLE_RING_BUFFER_HEADER followed by a circular audio data buffer.
    The shared memory can be mapped into user-mode address space via
    an IOCTL, enabling zero-copy audio data transfer between the kernel
    driver and the Cable Tauri application.

    Memory layout:
        [CABLE_RING_BUFFER_HEADER][...audio data buffer...]

    For render endpoints: Driver writes to ring buffer, app reads.
    For capture endpoints: App writes to ring buffer, driver reads.

--*/

#ifndef _CABLE_RING_BUFFER_H_
#define _CABLE_RING_BUFFER_H_

#include "definitions.h"

// Default ring buffer size for audio data (excluding header).
// 64KB provides ~340ms at 48kHz/16bit/stereo, which is plenty of headroom.
#define CABLE_DEFAULT_RING_BUFFER_SIZE  (64 * 1024)

// Pool tag for ring buffer allocations
#define CABLE_RINGBUF_POOLTAG   'bRcC'

class CableRingBuffer
{
public:
    CableRingBuffer();
    ~CableRingBuffer();

    //
    // Initialize the ring buffer with the specified data buffer size.
    // Allocates a contiguous kernel buffer of (sizeof(CABLE_RING_BUFFER_HEADER) + DataBufferSize).
    // Creates an MDL for the allocation so it can be mapped to user-mode later.
    //
    _IRQL_requires_max_(PASSIVE_LEVEL)
    NTSTATUS Initialize(
        _In_ ULONG DataBufferSize
    );

    //
    // Tear down the ring buffer, freeing all allocations.
    //
    _IRQL_requires_max_(PASSIVE_LEVEL)
    VOID Cleanup();

    //
    // Write audio data into the ring buffer (used by render path).
    // The driver calls this from ReadBytes() to push rendered audio
    // into the ring buffer for the user-mode app to consume.
    //
    // Runs at DISPATCH_LEVEL (called from timer DPC via UpdatePosition).
    //
    _IRQL_requires_max_(DISPATCH_LEVEL)
    VOID Write(
        _In_reads_bytes_(Length) const BYTE* Data,
        _In_ ULONG Length
    );

    //
    // Read audio data from the ring buffer (used by capture path).
    // The driver calls this from WriteBytes() to pull audio data
    // that the user-mode app has written, and copies it into the
    // WaveRT DMA buffer. Returns the number of bytes actually read.
    // If insufficient data is available, remaining bytes are zeroed.
    //
    // Runs at DISPATCH_LEVEL (called from timer DPC via UpdatePosition).
    //
    _IRQL_requires_max_(DISPATCH_LEVEL)
    ULONG Read(
        _Out_writes_bytes_(Length) BYTE* Buffer,
        _In_ ULONG Length
    );

    //
    // Map the shared memory into the calling process's user-mode
    // address space. Returns the user-mode base address.
    // The caller is responsible for calling Unmap() when done.
    //
    _IRQL_requires_max_(PASSIVE_LEVEL)
    NTSTATUS MapToUserMode(
        _Out_ PVOID* UserAddress
    );

    //
    // Unmap a previously mapped user-mode view.
    //
    _IRQL_requires_max_(PASSIVE_LEVEL)
    VOID UnmapFromUserMode(
        _In_ PVOID UserAddress
    );

    //
    // Get the total allocation size (header + data buffer).
    //
    ULONG GetTotalSize() const { return m_TotalSize; }

    //
    // Get the data buffer size (excluding header).
    //
    ULONG GetDataBufferSize() const { return m_DataBufferSize; }

    //
    // Get a pointer to the header (kernel address).
    //
    PCABLE_RING_BUFFER_HEADER GetHeader() const
    {
        return m_pHeader;
    }

    //
    // Check if the ring buffer has been initialized.
    //
    BOOLEAN IsInitialized() const { return (m_pHeader != NULL); }

    //
    // External lifetime management for callers caching this object.
    //
    LONG AddReference();
    LONG ReleaseReference();
    LONG GetReferenceCount() const;

    //
    // Reset read/write indices and status to zero.
    //
    _IRQL_requires_max_(DISPATCH_LEVEL)
    VOID Reset();

private:
    // Kernel virtual address of the shared memory allocation
    // Points to the CABLE_RING_BUFFER_HEADER at the start
    PCABLE_RING_BUFFER_HEADER   m_pHeader;

    // Pointer to the start of the audio data (immediately after header)
    BYTE*                       m_pDataBuffer;

    // MDL describing the shared memory for user-mode mapping
    PMDL                        m_pMdl;

    // Total size of the allocation (header + data buffer)
    ULONG                       m_TotalSize;

    // Size of just the data buffer portion
    ULONG                       m_DataBufferSize;

    // External reference count for safe object lifetime coordination.
    volatile LONG               m_ReferenceCount;
};

#endif // _CABLE_RING_BUFFER_H_
