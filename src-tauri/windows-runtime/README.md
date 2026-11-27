# Windows inference runtime

`DirectML.dll` comes from the ONNX Runtime dependency download and is copied
here for installer packaging. It is a generated artifact, ignored by Git.
Use `scripts/build-windows.ps1` on Windows to prepare it and build the installer.
Windows currently uses the CPU inference path; this DLL is required by the
upstream runtime distribution, not a claim of GPU acceleration.
