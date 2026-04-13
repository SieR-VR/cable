/*++

Module Name:

    CableRingBuffer.cpp

Abstract:

    Implementation of the CableRingBuffer class.

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

#include "definitions.h"
#include "CableRingBuffer.h"

#pragma warning (disable : 4127)

//=============================================================================
// CableRingBuffer
//=============================================================================

CableRingBuffer::CableRingBuffer()
    : m_pHeader(NULL)
    , m_pDataBuffer(NULL)
    , m_pMdl(NULL)
    , m_TotalSize(0)
    , m_DataBufferSize(0)
    , m_ReferenceCount(0)
{
}

CableRingBuffer::~CableRingBuffer()
{
    Cleanup();
}

//=============================================================================
#pragma code_seg("PAGE")
_Use_decl_annotations_
NTSTATUS
CableRingBuffer::Initialize(
    _In_ ULONG DataBufferSize
)
/*++

Routine Description:

    Allocates a contiguous kernel memory region for the ring buffer.
    The region consists of a CABLE_RING_BUFFER_HEADER followed by
    DataBufferSize bytes of audio data buffer.

    An MDL is created for the allocation to enable later user-mode mapping.

Arguments:

    DataBufferSize - Size of the audio data buffer in bytes (excluding header).

Return Value:

    STATUS_SUCCESS on success, or an appropriate error status.

--*/
{
    PAGED_CODE();

    DPF_ENTER(("[CableRingBuffer::Initialize]"));

    if (m_pHeader != NULL)
    {
        DPF(D_ERROR, ("CableRingBuffer::Initialize: already initialized"));
        return STATUS_ALREADY_INITIALIZED;
    }

    if (DataBufferSize == 0)
    {
        DPF(D_ERROR, ("CableRingBuffer::Initialize: zero buffer size"));
        return STATUS_INVALID_PARAMETER;
    }

    m_DataBufferSize = DataBufferSize;
    m_TotalSize = sizeof(CABLE_RING_BUFFER_HEADER) + DataBufferSize;

    //
    // Allocate contiguous nonpaged memory for the shared region.
    // Using ExAllocatePool2 with POOL_FLAG_NON_PAGED ensures the memory
    // is accessible at DISPATCH_LEVEL (for the timer DPC audio path)
    // and can be locked/mapped to user-mode.
    //
    PVOID pBuffer = ExAllocatePool2(
        POOL_FLAG_NON_PAGED,
        m_TotalSize,
        CABLE_RINGBUF_POOLTAG
    );

    if (pBuffer == NULL)
    {
        DPF(D_ERROR, ("CableRingBuffer::Initialize: ExAllocatePool2 failed (%u bytes)", m_TotalSize));
        m_TotalSize = 0;
        m_DataBufferSize = 0;
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    //
    // Zero the entire allocation.
    //
    RtlZeroMemory(pBuffer, m_TotalSize);

    //
    // Set up the header and data pointers.
    //
    m_pHeader = (PCABLE_RING_BUFFER_HEADER)pBuffer;
    m_pDataBuffer = (BYTE*)pBuffer + sizeof(CABLE_RING_BUFFER_HEADER);

    //
    // Initialize the header fields.
    //
    m_pHeader->WriteIndex = 0;
    m_pHeader->ReadIndex = 0;
    m_pHeader->BufferSize = DataBufferSize;
    m_pHeader->Status = CABLE_RING_BUFFER_STATUS_OK;
    m_pHeader->SampleRate = 48000;
    m_pHeader->Channels = 2;
    m_pHeader->BitsPerSample = 32;
    m_pHeader->DataType = CableAudioDataFloat32;
    m_pHeader->Magic = CABLE_RING_BUFFER_MAGIC;

    //
    // Create an MDL for the allocation so we can map it to user-mode.
    //
    m_pMdl = IoAllocateMdl(
        pBuffer,
        m_TotalSize,
        FALSE,  // Not secondary buffer
        FALSE,  // Don't charge quota
        NULL    // No IRP association
    );

    if (m_pMdl == NULL)
    {
        DPF(D_ERROR, ("CableRingBuffer::Initialize: IoAllocateMdl failed"));
        ExFreePoolWithTag(pBuffer, CABLE_RINGBUF_POOLTAG);
        m_pHeader = NULL;
        m_pDataBuffer = NULL;
        m_TotalSize = 0;
        m_DataBufferSize = 0;
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    //
    // Build the MDL to describe the non-paged pool allocation.
    // MmBuildMdlForNonPagedPool fills in the PFN array in the MDL
    // for memory that is already locked (non-paged pool is always locked).
    //
    MmBuildMdlForNonPagedPool(m_pMdl);

    DPF(D_TERSE, ("CableRingBuffer::Initialize: OK total=%u data=%u header=%u",
        m_TotalSize, m_DataBufferSize, (ULONG)sizeof(CABLE_RING_BUFFER_HEADER)));

    return STATUS_SUCCESS;
}

//=============================================================================
#pragma code_seg("PAGE")
VOID
CableRingBuffer::Cleanup()
/*++

Routine Description:

    Frees all allocations associated with the ring buffer.
    Safe to call multiple times or on an uninitialized object.

--*/
{
    PAGED_CODE();

    DPF_ENTER(("[CableRingBuffer::Cleanup]"));

    if (m_pMdl != NULL)
    {
        IoFreeMdl(m_pMdl);
        m_pMdl = NULL;
    }

    if (m_pHeader != NULL)
    {
        ExFreePoolWithTag(m_pHeader, CABLE_RINGBUF_POOLTAG);
        m_pHeader = NULL;
        m_pDataBuffer = NULL;
    }

    m_TotalSize = 0;
    m_DataBufferSize = 0;
}

//=============================================================================
#pragma code_seg()
_Use_decl_annotations_
VOID
CableRingBuffer::Write(
    _In_reads_bytes_(Length) const BYTE* Data,
    _In_ ULONG Length
)
/*++

Routine Description:

    Writes audio data into the ring buffer. Used by the render path:
    the driver reads from the WaveRT DMA buffer and writes into this
    ring buffer for the user-mode app to consume.

    Runs at DISPATCH_LEVEL (called from timer DPC via UpdatePosition/ReadBytes).

    This performs a simple producer write: advances WriteIndex after copying.
    No locking against the consumer (user-mode app) is needed because the
    producer only writes WriteIndex and the consumer only writes ReadIndex.
    Both indices are naturally aligned and written atomically.

Arguments:

    Data   - Pointer to the source audio data.
    Length - Number of bytes to write.

--*/
{
    if (m_pHeader == NULL || Length == 0)
    {
        return;
    }

    ULONG bufferSize = m_pHeader->BufferSize;
    if (bufferSize == 0)
    {
        return;
    }

    ULONG writeIdx = (ULONG)(m_pHeader->WriteIndex % bufferSize);
    ULONG remaining = Length;
    const BYTE* src = Data;

    while (remaining > 0)
    {
        ULONG chunk = min(remaining, bufferSize - writeIdx);
        RtlCopyMemory(m_pDataBuffer + writeIdx, src, chunk);
        writeIdx = (writeIdx + chunk) % bufferSize;
        src += chunk;
        remaining -= chunk;
    }

    //
    // Update the write index atomically. Use a full memory barrier
    // to ensure all data writes are visible before the index update.
    //
    KeMemoryBarrier();
    m_pHeader->WriteIndex += Length;
}

//=============================================================================
#pragma code_seg()
_Use_decl_annotations_
ULONG
CableRingBuffer::Read(
    _Out_writes_bytes_(Length) BYTE* Buffer,
    _In_ ULONG Length
)
/*++

Routine Description:

    Reads audio data from the ring buffer. Used by the capture path:
    the user-mode app writes audio data into this ring buffer, and the
    driver reads it out to fill the WaveRT DMA buffer.

    If insufficient data is available, the remaining bytes are zeroed
    (silence fill) to prevent glitches.

    Runs at DISPATCH_LEVEL (called from timer DPC via UpdatePosition/WriteBytes).

Arguments:

    Buffer - Destination buffer for the audio data.
    Length - Number of bytes to read.

Return Value:

    Number of bytes actually read from the ring buffer (remainder is silence-filled).

--*/
{
    if (m_pHeader == NULL || Length == 0)
    {
        if (Length > 0)
        {
            RtlZeroMemory(Buffer, Length);
        }
        return 0;
    }

    ULONG bufferSize = m_pHeader->BufferSize;
    if (bufferSize == 0)
    {
        RtlZeroMemory(Buffer, Length);
        return 0;
    }

    //
    // Calculate how many bytes are available for reading.
    // WriteIndex >= ReadIndex is the normal case.
    // Both are monotonically increasing; available = WriteIndex - ReadIndex.
    //
    ULONGLONG writeIndex = m_pHeader->WriteIndex;
    KeMemoryBarrier(); // Ensure we read data after reading the write index
    ULONGLONG readIndex = m_pHeader->ReadIndex;

    ULONGLONG available = writeIndex - readIndex;

    // Clamp available to buffer size to handle extreme wrap cases
    if (available > bufferSize)
    {
        // Overrun: app wrote more than buffer can hold, data was lost.
        // Reset read index to catch up.
        m_pHeader->Status = CABLE_RING_BUFFER_STATUS_OVERRUN;
        readIndex = writeIndex - bufferSize;
        m_pHeader->ReadIndex = readIndex;
        available = bufferSize;
    }

    ULONG bytesToRead = (ULONG)min((ULONGLONG)Length, available);
    ULONG bytesRead = 0;

    if (bytesToRead > 0)
    {
        ULONG readIdx = (ULONG)(readIndex % bufferSize);
        ULONG remaining = bytesToRead;
        BYTE* dst = Buffer;

        while (remaining > 0)
        {
            ULONG chunk = min(remaining, bufferSize - readIdx);
            RtlCopyMemory(dst, m_pDataBuffer + readIdx, chunk);
            readIdx = (readIdx + chunk) % bufferSize;
            dst += chunk;
            remaining -= chunk;
        }

        bytesRead = bytesToRead;

        //
        // Update the read index atomically.
        //
        KeMemoryBarrier();
        m_pHeader->ReadIndex += bytesToRead;
    }

    //
    // Silence-fill any remaining bytes that were not available.
    //
    if (bytesRead < Length)
    {
        RtlZeroMemory(Buffer + bytesRead, Length - bytesRead);

        // Signal underrun if we had to pad with silence
        if (bytesRead == 0)
        {
            m_pHeader->Status = CABLE_RING_BUFFER_STATUS_UNDERRUN;
        }
    }

    return bytesRead;
}

//=============================================================================
#pragma code_seg("PAGE")
_Use_decl_annotations_
NTSTATUS
CableRingBuffer::MapToUserMode(
    _Out_ PVOID* UserAddress
)
/*++

Routine Description:

    Maps the shared memory region (header + data buffer) into the calling
    process's user-mode address space.

    The caller must call UnmapFromUserMode() to release the mapping.

Arguments:

    UserAddress - Receives the user-mode base address of the mapped region.

Return Value:

    STATUS_SUCCESS on success, or an appropriate error status.

--*/
{
    PAGED_CODE();

    DPF_ENTER(("[CableRingBuffer::MapToUserMode]"));

    if (UserAddress == NULL)
    {
        return STATUS_INVALID_PARAMETER;
    }

    *UserAddress = NULL;

    if (m_pMdl == NULL || m_pHeader == NULL)
    {
        DPF(D_ERROR, ("CableRingBuffer::MapToUserMode: not initialized"));
        return STATUS_INVALID_DEVICE_STATE;
    }

    //
    // Map the MDL into user-mode address space.
    // MmMapLockedPagesSpecifyCache with UserMode maps the locked pages
    // into the current process's virtual address space.
    //
    __try
    {
        PVOID pUserAddr = MmMapLockedPagesSpecifyCache(
            m_pMdl,
            UserMode,
            MmCached,
            NULL,       // Let MM choose the user-mode address
            FALSE,      // Don't bug-check on failure
            NormalPagePriority | MdlMappingNoExecute
        );

        if (pUserAddr == NULL)
        {
            DPF(D_ERROR, ("CableRingBuffer::MapToUserMode: MmMapLockedPagesSpecifyCache returned NULL"));
            return STATUS_INSUFFICIENT_RESOURCES;
        }

        *UserAddress = pUserAddr;

        DPF(D_TERSE, ("CableRingBuffer::MapToUserMode: mapped at %p (size=%u)",
            pUserAddr, m_TotalSize));

        return STATUS_SUCCESS;
    }
    __except(EXCEPTION_EXECUTE_HANDLER)
    {
        DPF(D_ERROR, ("CableRingBuffer::MapToUserMode: exception 0x%x",
            GetExceptionCode()));
        return STATUS_ACCESS_VIOLATION;
    }
}

//=============================================================================
#pragma code_seg("PAGE")
_Use_decl_annotations_
VOID
CableRingBuffer::UnmapFromUserMode(
    _In_ PVOID UserAddress
)
/*++

Routine Description:

    Unmaps a previously mapped user-mode view of the ring buffer.

Arguments:

    UserAddress - The user-mode address returned by MapToUserMode().

--*/
{
    PAGED_CODE();

    DPF_ENTER(("[CableRingBuffer::UnmapFromUserMode]"));

    if (UserAddress == NULL || m_pMdl == NULL)
    {
        return;
    }

    MmUnmapLockedPages(UserAddress, m_pMdl);

    DPF(D_TERSE, ("CableRingBuffer::UnmapFromUserMode: unmapped %p", UserAddress));
}

//=============================================================================
#pragma code_seg()
VOID
CableRingBuffer::Reset()
/*++

Routine Description:

    Resets the ring buffer read/write indices and status to zero.
    Call this when the stream transitions to STOP state.

    Safe to call at DISPATCH_LEVEL.

--*/
{
    if (m_pHeader != NULL)
    {
        m_pHeader->WriteIndex = 0;
        m_pHeader->ReadIndex = 0;
        m_pHeader->Status = CABLE_RING_BUFFER_STATUS_OK;
        KeMemoryBarrier();
    }
}

//=============================================================================
#pragma code_seg()
LONG
CableRingBuffer::AddReference()
{
    return InterlockedIncrement(&m_ReferenceCount);
}

//=============================================================================
#pragma code_seg()
LONG
CableRingBuffer::ReleaseReference()
{
    LONG current = InterlockedCompareExchange(&m_ReferenceCount, 0, 0);

    while (current > 0)
    {
        LONG updated = InterlockedCompareExchange(&m_ReferenceCount, current - 1, current);
        if (updated == current)
        {
            return (current - 1);
        }
        current = updated;
    }

    DPF(D_TERSE, ("CableRingBuffer::ReleaseReference called with zero reference count"));
    return 0;
}

//=============================================================================
#pragma code_seg()
LONG
CableRingBuffer::GetReferenceCount() const
{
    return InterlockedCompareExchange((volatile LONG*)&m_ReferenceCount, 0, 0);
}
