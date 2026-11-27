param(
  # Keep the original CPU-only package path for machines without CUDA.
  [switch]$CpuOnly
)

$ErrorActionPreference = "Stop"

$cudaPath = $env:CUDA_PATH
$nvcc = if ($cudaPath) { Join-Path $cudaPath "bin\nvcc.exe" } else { $null }
if (-not $cudaPath) {
  $nvccCommand = Get-Command nvcc.exe -ErrorAction SilentlyContinue
  if ($nvccCommand) {
    $nvcc = $nvccCommand.Source
    $cudaPath = Split-Path (Split-Path $nvcc -Parent) -Parent
  }
}
$useCuda = -not $CpuOnly -and $nvcc -and (Test-Path $nvcc)
$features = @()

if ($useCuda) {
  $versionOutput = (& $nvcc --version 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0 -or $versionOutput -notmatch "release\s+(\d+)") {
    throw "Could not determine the CUDA toolkit version from nvcc."
  }
  $cudaMajor = [int]$Matches[1]
  # ort 2.0.0-rc.13 currently ships its Windows CUDA provider for CUDA 13.
  if ($cudaMajor -ne 13) {
    throw "The combined Whisper/Parakeet CUDA build currently requires CUDA 13.x. Use -CpuOnly for a CPU build."
  }
  $env:ORT_CUDA_VERSION = "13"
  # A T4-only build (CUDAARCHS=75) crashes on newer RTX cards because the
  # executable has no compatible kernel image. Cover the desktop GPUs Clide
  # targets: Tesla T4/RTX 20 (75), RTX 30 (86), RTX 40 (89), RTX 50 (120).
  # Users can still override CUDAARCHS, e.g. "all-major" for data-center cards.
  if (-not $env:CUDAARCHS) { $env:CUDAARCHS = "75;86;89;120" }
  $features = @("--features", "cuda")
  Write-Host "CUDA $cudaMajor detected: building GPU-accelerated Whisper and Parakeet for $env:CUDAARCHS."
} else {
  Write-Host "Building the CPU-only Windows package."
}

# ort-sys currently copies runtime files relative to Cargo's build directory.
foreach ($profile in @("debug", "release")) {
  foreach ($dir in @("examples", "deps")) {
    New-Item -ItemType Directory -Force "src-tauri/target/$profile/build/$dir" | Out-Null
  }
}

npm ci
if ($LASTEXITCODE -ne 0) { throw "npm ci failed" }

# Tauri validates the Windows resource glob during `cargo check`, before the
# real runtime DLLs exist on a fresh checkout. A placeholder keeps the glob
# valid; it is deleted below and replaced with the actual runtime files before
# the bundle is built.
New-Item -ItemType Directory -Force "src-tauri/windows-runtime" | Out-Null
if (-not (Get-ChildItem "src-tauri/windows-runtime" -Filter "*.dll" -File)) {
  New-Item -ItemType File -Force "src-tauri/windows-runtime/placeholder.dll" | Out-Null
}

cargo check --manifest-path src-tauri/Cargo.toml @features
if ($LASTEXITCODE -ne 0) { throw "cargo check failed" }

New-Item -ItemType Directory -Force "src-tauri/windows-runtime" | Out-Null
# Do not let a previous CUDA build leak provider DLLs into a CPU-only package.
Get-ChildItem "src-tauri/windows-runtime" -Filter "*.dll" -File | Remove-Item -Force

function Copy-RuntimeDll {
  param(
    [string]$Name,
    [string]$SourceRoot = "src-tauri/target/debug"
  )
  $dll = Get-ChildItem $SourceRoot -Filter $Name -Recurse | Select-Object -First 1
  if (-not $dll) { throw "Required runtime DLL was not found: $Name" }
  Copy-Item $dll.FullName "src-tauri/windows-runtime/$Name" -Force
}

Copy-RuntimeDll "DirectML.dll"
if ($useCuda) {
  Copy-RuntimeDll "onnxruntime_providers_cuda.dll"
  Copy-RuntimeDll "onnxruntime_providers_shared.dll"
}

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$crt = Get-ChildItem "$vs/VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT" -Directory | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $crt) { throw "MSVC x64 redistributable files are missing" }
Copy-Item "$($crt.FullName)/*.dll" "src-tauri/windows-runtime/" -Force

if ($useCuda) {
  # nvcuda.dll is the NVIDIA driver and must not be bundled. These toolkit
  # runtime DLLs are redistributable beside the application.
  foreach ($name in @("cudart64_13.dll", "cublas64_13.dll", "cublasLt64_13.dll")) {
    $dll = @(
      (Join-Path $cudaPath "bin/$name"),
      (Join-Path $cudaPath "bin/x64/$name")
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $dll) { throw "CUDA 13 runtime DLL was not found: $name" }
    Copy-Item $dll "src-tauri/windows-runtime/$name" -Force
  }
}

$tauriArgs = @("build", "--bundles", "nsis", "--no-sign", "--ci")
if ($useCuda) { $tauriArgs += @("--features", "cuda") }
npm run tauri -- @tauriArgs
if ($LASTEXITCODE -ne 0) { throw "Windows installer build failed" }
