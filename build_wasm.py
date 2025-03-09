from pathlib import Path
import subprocess
import json
import shutil

ROOT_DIR=Path(__file__).parent.resolve()
WASM_BUILD_ROOT=ROOT_DIR / 'wasm-build'

def main():
    shutil.rmtree(WASM_BUILD_ROOT, ignore_errors=True)
    code_dir = ROOT_DIR / 'code'
    for path in code_dir.glob('*'):
        if path.is_dir():
            rel_path = path.relative_to(code_dir)
            out_dir = WASM_BUILD_ROOT / rel_path

            subprocess.check_call([
                'wasm-pack',
                'build',
                path,
                '--target',
                'web',
                '--out-dir',
                out_dir,
                '--out-name',
                'wasm-package',
                '--no-typescript', # to not generate ts files
                '--no-pack', # do not generate package.json
            ])
            shutil.copyfile(ROOT_DIR / 'index.html', out_dir / 'index.html')

            gitignore = out_dir / '.gitignore'
            gitignore.unlink() # whole build dir is already ignored


if __name__ == '__main__':
    main()
