#!/usr/bin/env python3

import argparse
from enum import Enum
import os
import subprocess
import shutil
import shlex
import math
from pathlib import Path
from sys import builtin_module_names

LLVM_VERSION = '19.1.7'
ICU_VERSION = '76.1'

here = Path(__file__).parent.resolve()

NONE = 0
CLANG = 1
GCC = 2
MSVC = 3

force = None

parser = argparse.ArgumentParser()

parser.add_argument('--no-ninja', action='store_true', help='Do not use Ninja if present')
parser.add_argument('--clang', action='store_true', help='Make sure the code is compiled using Clang ')
parser.add_argument('--gcc', action='store_true', help='Make sure the code is compiled using GCC ')
parser.add_argument('--msvc', action='store_true', help='Make sure the code is compiled using the Microsoft Visual C++ compiler')
parser.add_argument('--target', action='append', help='CPU target to support. Can be specified multiple times.')
parser.add_argument('-j', '--jobs', help='The maximum amount of jobs that build in parallel')
parser.add_argument('--no-system-llvm', action='store_true', help='Use a local version of the LLVM compiler framework')

args = parser.parse_args()

if args.clang:
    force = CLANG
elif args.gcc:
    force = GCC

cache_dir = here / '.cache' / 'bolt-build'
download_dir = cache_dir / 'downloads'
source_dir = cache_dir / 'source'
binary_dir = cache_dir / 'opt'
build_dir = cache_dir / 'build'
llvm_source_dir = source_dir / 'llvm'
llvm_install_dir = binary_dir / 'llvm'
llvm_build_dir = build_dir / 'llvm'
icu_source_dir = source_dir / 'icu'
icu_install_dir = binary_dir / 'icu'
icu_build_dir = build_dir / 'icu'
bolt_source_dir = here
bolt_build_dir = build_dir / 'bolt'

def newer(a: Path, b: Path) -> bool:
    def min_time(path: Path) -> float:
        if not path.exists():
            return math.inf
        if path.is_dir():
            min = math.inf
            for p in path.iterdir():
                m = min_time(p)
                if m < min:
                    min = m
            return min
        return path.stat().st_mtime
    def max_time(path: Path) -> float:
        if not path.exists():
            return 0
        if path.is_dir():
            max = 0
            for p in path.iterdir():
                m = min_time(p)
                if m > max:
                    max = m
            return max
        return path.stat().st_mtime
    return min_time(a) > max_time(b)

class CMakeGenerator(Enum):
    ninja = 'Ninja'
    make = 'Unix Makefiles'

type CMakeValue = None | bool | int | str

def cmake_encode(value: CMakeValue):
    if isinstance(value, str):
        return shlex.quote(value)
    if value == True:
        return 'ON'
    if value == False:
        return 'OFF'
    if isinstance(value, int):
        return str(value);
    raise NotImplementedError()

def spawn(cmd: list[str], *args, **kwargs):
    print(' '.join(str(x) for x in cmd))
    subprocess.run(cmd, *args, check=True, **kwargs)

def shell(cmd: str, *args, **kwargs):
    print(cmd)
    subprocess.run(cmd, shell=True, *args, check=True, **kwargs)

def stdout(argv: list[str], *args, **kwargs) -> str:
    return subprocess.run(argv, check=True, stdout=subprocess.PIPE, *args, **kwargs).stdout.decode('utf-8').strip()

def cmake(
    src_dir: Path,
    build_dir: Path,
    generator: CMakeGenerator | None = None,
    defines: dict[str, CMakeValue] | None = None,
    compile_commands = True,
):
    if generator is None and ninja_path is not None:
        generator = CMakeGenerator.ninja
    if defines is None:
        defines = dict()
    argv = [
        'cmake',
        '-S',
        src_dir,
        '-B', build_dir,
        ]
    if generator is not None:
        argv.extend(['-G', generator.value])
    if cxx_path is not None:
        argv.append(f'-DCMAKE_CXX_COMPILER={cmake_encode(str(cxx_path))}')
    if c_path is not None:
        argv.append(f'-DCMAKE_C_COMPILER={cmake_encode(str(c_path))}')
    if compile_commands:
        argv.append('-DCMAKE_EXPORT_COMPILE_COMMANDS=ON')
    for k, v in defines.items():
        argv.append(f'-D{k}={cmake_encode(v)}')
    spawn(argv)
    compile_commands_json_path = here / 'compile_commands.json';
    if compile_commands and not compile_commands_json_path.exists():
        compile_commands_json_path.symlink_to(build_dir / 'compile_commands.json')

def build(*targets: str, build_dir: Path, jobs: int | None = None) -> None:
    args = [ 'cmake', '--build', build_dir ]
    if jobs is not None:
        args.extend(['-j', str(jobs) ])
    for target in targets:
        args.extend([ '-t', target ])
    spawn(args)

def mkdirp(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)

def touch(path: Path) -> None:
    with open(path, 'w'):
        pass

def download_llvm(version: str):
    fname = f'llvmorg-{version}.tar.gz'
    downloaded_path = download_dir / (fname + '.downloaded')
    extracted_path = download_dir / (fname + '.extracted')
    if not downloaded_path.exists():
        mkdirp(download_dir)
        shell(f'wget --continue https://github.com/llvm/llvm-project/archive/refs/tags/llvmorg-{version}.tar.gz', cwd=download_dir)
        touch(downloaded_path)
    if not extracted_path.exists():
        mkdirp(llvm_source_dir)
        shell(f'tar -xf {fname} --directory {llvm_source_dir} --strip-components=1', cwd=download_dir)
        touch(extracted_path)

def build_llvm(target_archs: list[str], jobs: int | None = None):

    download_llvm(LLVM_VERSION)

    cmake_generated_path = llvm_source_dir / '.cmake-generated'
    cmake_built_path = llvm_source_dir / '.cmake-built'
    cmake_installed_path = llvm_source_dir / '.cmake-installed'

    if not cmake_generated_path.exists():
        cmake(
            llvm_source_dir / 'llvm',
            llvm_build_dir,
            defines={
                'CMAKE_INSTALL_PREFIX': str(llvm_install_dir),
                'CMAKE_BUILD_TYPE': 'Release',
                'LLVM_ENABLE_ASSERTIONS': True,
                'LLVM_ENABLE_PROJECTS': '',
                'LLVM_TARGETS_TO_BUILD': ';'.join(target_archs),
                'LLVM_OPTIMIZED_TABLEGEN': True,
                'CMAKE_C_COMPILER': str(c_path),
                'CMAKE_CXX_COMPILER': str(cxx_path),
            }
        )
        touch(cmake_generated_path)

    if not cmake_built_path.exists():
        build_cmd = 'make' if ninja_path is None else 'ninja'
        build_argv = [ build_cmd ]
        if jobs is not None:
            build_argv.extend([ '-j', str(jobs) ])
        spawn(build_argv, cwd=llvm_build_dir)
        touch(cmake_built_path)

    if not cmake_installed_path.exists():
        spawn([ 'cmake', '--install', str(llvm_build_dir) ])
        touch(cmake_installed_path)

def ninja(targets: list[str], cwd: Path | None = None) -> None:
    argv = [ str(ninja_path) ]
    argv.extend(targets)
    if cwd is not None:
        argv.extend([ '-C', str(cwd) ])
    spawn(argv)

def build_bolt(llvm_root: Path, icu_root: Path) -> None:

    if newer(bolt_source_dir / 'CMakeLists.txt', bolt_build_dir):

        print(icu_root)
        defines: dict[str, CMakeValue] = {
            'CMAKE_EXPORT_COMPILE_COMMANDS': True,
            'CMAKE_BUILD_TYPE': 'Debug',
            'BOLT_ENABLE_TESTS': True,
            'ZEN_ENABLE_TESTS': False,
            'LLVM_ROOT': str(llvm_root),
            'ICU_ROOT': str(icu_root),
            #'LLVM_CONFIG': str(llvm_config_path),
            #'LLVM_TARGETS_TO_BUILD': 'X86',
        }
        defines['CMAKE_C_COMPILER'] = str(c_path)
        defines['CMAKE_CXX_COMPILER'] = str(cxx_path)
        cmake(
            bolt_source_dir,
            bolt_build_dir,
            defines=defines,
        )

    build('bolt', build_dir=bolt_build_dir)

def download_icu(version: str) -> None:
    fname = f'icu4c-{version.replace('.', '_')}-src.tgz'
    downloaded_path = download_dir / (fname + '.downloaded')
    extracted_path = download_dir / (fname + '.extracted')
    if not downloaded_path.exists():
        mkdirp(download_dir)
        shell(f'wget --continue https://github.com/unicode-org/icu/releases/download/release-{version.replace('.', '-')}/icu4c-{version.replace('.', '_')}-src.tgz', cwd=download_dir)
        touch(downloaded_path)
    if not extracted_path.exists():
        mkdirp(icu_source_dir)
        shell(f'tar -xf {fname} --directory {icu_source_dir} --strip-components=1', cwd=download_dir)
        touch(extracted_path)

def build_icu(version: str) -> None:
    download_icu(version)
    mkdirp(icu_build_dir)
    env = dict(os.environ)
    env['CC'] = str(c_path)
    env['CXX'] = str(cxx_path)
    env['CXXFLAGS'] = '-std=c++17'
    #env['CPPFLAGS'] = '-DUNISTR_FROM_CHAR_EXPLICIT=explicit -DUNISTR_FROM_STRING_EXPLICIT=explicit -DU_NO_DEFAULT_INCLUDE_UTF_HEADERS=1 -DU_HIDE_OBSOLETE_UTF_OLD_H=1'
    env['CPPFLAGS'] = '-DU_NO_DEFAULT_INCLUDE_UTF_HEADERS=1 -DU_HIDE_OBSOLETE_UTF_OLD_H=1'
    configured_path = icu_source_dir / '.configured'
    if not configured_path.exists():
        spawn([ f'{icu_source_dir}/source/runConfigureICU', 'Linux', '--enable-static', '--prefix', str(icu_install_dir) ], cwd=icu_build_dir, env=env)
        touch(configured_path)
    installed_path = icu_source_dir / '.installed'
    if not installed_path.exists():
        shell(f'make install -j{os.cpu_count()}', cwd=icu_build_dir, env=env)
        touch(installed_path)

enable_ninja = not args.no_ninja

ninja_path = enable_ninja and shutil.which('ninja')

def detect_compilers() -> tuple[Path, Path] | None:
    if os.name == 'posix':
        cxx_path = os.environ.get('CXX')
        c_path = os.environ.get('CC')
        if c_path is not None and cxx_path is not None:
            return Path(c_path).absolute(), Path(cxx_path).absolute()
        for suffix in [ '', '-19', '-18' ]:
            clang_c_path = shutil.which(f'clang{suffix}')
            clang_cxx_path = shutil.which(f'clang++{suffix}')
            if clang_c_path is not None and clang_cxx_path is not None and (force == NONE or force == CLANG):
                return Path(clang_c_path), Path(clang_cxx_path)
        gcc_c_path = shutil.which('gcc')
        gcc_cxx_path = shutil.which('g++')
        if gcc_c_path is not None and gcc_cxx_path is not None and (force == NONE or force == GCC):
            return Path(gcc_c_path), Path(gcc_cxx_path)
        c_path = shutil.which('cc')
        cxx_path = shutil.which('c++')
        if c_path is not None and cxx_path is not None:
            print("Warning: falling back to default system compiler. This may not be what you asked.")
            # TODO determine the compiler type and match with force
            return Path(c_path), Path(cxx_path)
    elif os.name == 'nt':
        msvc_path = shutil.which('cl.exe')
        if msvc_path is not None and (force == NONE or force == MSVC):
            return Path(msvc_path), Path(msvc_path)
        else:
            print("Error: could not detect C/C++ compiler")
            exit(1)
    else:
        print('Error: platform not supported right now')

result = detect_compilers()
if result is None:
    print('Error: no suitable compiler could be detected.')
    exit(1)
c_path, cxx_path = result

num_jobs = args.jobs
llvm_targets = []

if args.target is None:
    llvm_targets.append('host')
else:
    for target_spec in args.target:
        for target in target_spec.split(','):
            llvm_targets.append(target)

llvm_config_path = shutil.which('llvm-config')

if llvm_config_path is None or args.no_system_llvm:
    build_llvm(llvm_targets, jobs=num_jobs)
    llvm_config_path = llvm_install_dir / 'bin' / 'llvm-config'

llvm_root = Path(stdout([ str(llvm_config_path), '--cmakedir' ]))

build_icu(version=ICU_VERSION)

build_bolt(llvm_root=llvm_root, icu_root=icu_install_dir)

