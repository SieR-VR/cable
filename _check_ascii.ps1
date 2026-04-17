$files = @(
    'driver/Source/Main/common.cpp',
    'driver/Source/Main/adapter.cpp',
    'driver/Source/Filters/minipairs.h',
    'driver/Source/Inc/common.h',
    'driver/Source/Inc/cable_common.h'
)
foreach ($f in $files) {
    $bytes = [System.IO.File]::ReadAllBytes($f)
    $bad = @()
    for ($i=0; $i -lt $bytes.Length; $i++) {
        if ($bytes[$i] -gt 127) { $bad += $i }
    }
    if ($bad.Count -gt 0) {
        Write-Host "${f}: $($bad.Count) non-ASCII bytes"
    } else {
        Write-Host "${f}: OK"
    }
}
