@set arch=%1
@if "%arch%"=="" (
    @echo "Usage: build-win (x86_64/i686) [/A]"
    @exit /b 1
)

if not exist build mkdir build
if not exist target mkdir target

cargo build --target %arch%-pc-windows-msvc --release --message-format=json-render-diagnostics | jq -r "select(.out_dir) | select(.package_id | startswith(\"hedgehog-tui \")) | .out_dir" > .\target\out_dir_path_%arch%

rmdir /S /Q .\build\hedgehog-current-windows-%arch%
mkdir .\build\hedgehog-current-windows-%arch%

copy target\%arch%-pc-windows-msvc\release\hedgehog.exe build\hedgehog-current-windows-%arch%\
for /f "usebackq tokens=*" %%a in (`type .\target\out_dir_path_%arch%`) do xcopy /S /I %%a\config build\hedgehog-current-windows-%arch%\config

if "%2"=="/A" (
    cd build
	7z a -sdel hedgehog-current-windows-%arch%.zip hedgehog-current-windows-%arch%
	cd ..
)
