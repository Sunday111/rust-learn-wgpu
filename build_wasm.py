from pathlib import Path
import subprocess
import shutil

ROOT_DIR = Path(__file__).parent.resolve()
WASM_BUILD_ROOT = ROOT_DIR / "wasm-build"

def build_wasm_target(src_path:Path, out_dir:Path):
    subprocess.check_call(
        [
            "wasm-pack",
            *("build", src_path),
            *("--target", "web"),
            *("--out-dir", out_dir),
            *("--out-name", "wasm-package"),
            "--no-typescript",  # to not generate ts files
            "--no-pack",  # do not generate package.json
            "--dev",
        ]
    )

    gitignore = out_dir / ".gitignore"
    gitignore.unlink()  # whole build dir is already ignored

def main():
    shutil.rmtree(WASM_BUILD_ROOT, ignore_errors=True)
    build_wasm_target(ROOT_DIR / 'code/wasm-target', WASM_BUILD_ROOT)

    # copy html pages
    for html_path in (ROOT_DIR / 'html').glob('*'):
        shutil.copyfile(html_path, WASM_BUILD_ROOT / html_path.name)

    # copy dynamically loaded resources
    shutil.copytree(ROOT_DIR / 'code/klgl/res', WASM_BUILD_ROOT / 'res')


if __name__ == "__main__":
    main()
