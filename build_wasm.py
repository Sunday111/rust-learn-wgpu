from pathlib import Path
import subprocess
import json
import shutil

ROOT_DIR=Path(__file__).parent.resolve()
WASM_BUILD_ROOT=ROOT_DIR / 'wasm-build'

def main():
    shutil.rmtree(WASM_BUILD_ROOT, ignore_errors=True)
    with open(file=ROOT_DIR / 'wasm-targets.json', mode='r', encoding='utf-8') as file:
        for entry in json.load(file):
            package = entry['package']
            out_dir = WASM_BUILD_ROOT / package

            subprocess.check_call([
                'wasm-pack',
                'build',
                ROOT_DIR / 'code' / package,
                '--target',
                'web',
                '--out-dir',
                out_dir,
                '--out-name',
                'wasm-package'
            ])
            shutil.copyfile(ROOT_DIR / 'index.html', out_dir / 'index.html')

if __name__ == '__main__':
    main()
