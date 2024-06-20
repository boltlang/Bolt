#!/usr/bin/env python3

import argparse
from enum import Enum
from os import walk
import os
from re import A
import subprocess
import shutil
import shlex
import math
from pathlib import Path

LLVM_VERSION = '18.1.0'

here = Path(__file__).parent.resolve()

parser = argparse.ArgumentParser()

parser.add_argument('--no-ninja', action='store_true', help='Do not use Ninja if present')
parser.add_argument('--clang', action='store_true', help='Make sure the code is compiled using Clang ')
parser.add_argument('--gcc', action='store_true', help='Make sure the code is compiled using Clang ')
parser.add_argument('--msvc', action='store_true', help='Make sure the code is compiled using the Microsoft Visual C++ compiler')
parser.add_argument('--target', action='append', help='CPU target to support. Can be specified multiple times.')
parser.add_argument('-j', '--jobs', help='The maximum amount of jobs that build in parallel')

args = parser.parse_args()

cache_dir = here / '.cache' / 'bolt-build'
download_dir = cache_dir / 'downloads'
source_dir = cache_dir / 'source'
build_dir = cache_dir / 'build'
llvm_source_dir = source_dir / 'llvm'
llvm_build_dir = build_dir / 'llvm'
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
        src_dir,
        '-B', build_dir,
        ]
    if generator is not None:
        argv.extend(['-G', generator.value])
    if clang_cxx_path is not None:
        argv.append(f'-DCMAKE_CXX_COMPILER={cmake_encode(clang_cxx_path)}')
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

def download_llvm(version: str):
    download_dir.mkdir(parents=True, exist_ok=True)
    shell(f'wget https://github.com/llvm/llvm-project/archive/refs/tags/llvmorg-{version}.tar.gz', cwd=download_dir)
    shell(f'tar -xf llvmorg-{version}.tar.gz --directory {llvm_source_dir}', cwd=download_dir)

def build_llvm(target_archs: list[str], jobs: int | None = None):

    return # FIXME

    download_llvm(LLVM_VERSION)

    cmake(
        llvm_source_dir,
        llvm_build_dir,
        defines={
            'CMAKE_BUILD_TYPE': 'Release',
            'LLVM_ENABLE_ASSERTIONS': True,
            'LLVM_TARGETS_TO_BUILD': ';'.join(target_archs),
            'LLVM_OPTIMIZED_TABLEGEN': True
        }
    )

    build_cmd = 'make' if ninja_path is None else 'ninja'
    build_argv = [ build_cmd ]
    if jobs is not None:
        build_argv.extend([ '-j', str(jobs) ])

    spawn(build_argv, cwd=llvm_build_dir)

def ninja(targets: list[str], cwd: Path | None = None) -> None:
    argv = [ str(ninja_path) ]
    argv.extend(targets)
    if cwd is not None:
        argv.extend([ '-C', str(cwd) ])
    spawn(argv)

def build_bolt(c_path: str | None = None, cxx_path: str | None = None) -> None:

    if newer(bolt_source_dir / 'CMakeLists.txt', bolt_build_dir):

        defines = {
            'CMAKE_EXPORT_COMPILE_COMMANDS': True,
            'CMAKE_BUILD_TYPE': 'Debug',
            'BOLT_ENABLE_TESTS': True,
            'ZEN_ENABLE_TESTS': False,
            #'LLVM_CONFIG': str(llvm_config_path),
            'LLVM_TARGETS_TO_BUILD': 'X86',
        }
        if c_path is not None:
            defines['CMAKE_C_COMPILER'] = c_path
        if cxx_path is not None:
            defines['CMAKE_CXX_COMPILER'] = cxx_path
        cmake(
            bolt_source_dir,
            bolt_build_dir,
            defines=defines,
        )

    build('bolt', build_dir=bolt_build_dir)

enable_ninja = not args.no_ninja

NONE = 0
CLANG = 1
GCC = 2
MSVC = 3

force = NONE

ninja_path = enable_ninja and shutil.which('ninja')

c_path = None
cxx_path = None

if os.name == 'posix':
    clang_c_path = shutil.which('clang')
    clang_cxx_path = shutil.which('clang++')
    if clang_c_path is not None and clang_cxx_path is not None and (force == NONE or force == CLANG):
        c_path = clang_c_path
        cxx_path = clang_cxx_path
    else:
        for version in [ '18', '19' ]:
            clang_c_path = shutil.which(f'clang-{version}')
            clang_cxx_path = shutil.which(f'clang++-{version}')
            if clang_c_path is not None and clang_cxx_path is not None and (force == NONE or force == CLANG):
                c_path = clang_c_path
                cxx_path = clang_cxx_path
                break
    if c_path is None or cxx_path is None:
        gcc_c_path = shutil.which('gcc')
        gcc_cxx_path = shutil.which('g++')
        if gcc_c_path is not None and gcc_cxx_path is not None and (force == NONE or force == GCC):
            c_path = gcc_c_path
            cxx_path = gcc_cxx_path
        else:
            print('Going to use platform default compiler')
elif os.name == 'nt':
    msvc_path = shutil.which('cl.exe')
    if msvc_path is not None and (force == NONE or force == MSVC):
        c_path = msvc_path
        cxx_path = msvc_path
    else:
        print('Going to use platform default compiler')
else:
    print('Platform not supported right now')
    exit(1)

num_jobs = args.jobs
llvm_targets = []

if args.target is None:
    llvm_targets.append('host')
else:
    for target_spec in args.target:
        for target in target_spec.split(','):
            llvm_targets.append(target)

llvm_config_path = shutil.which('llvm-config-18')

if llvm_config_path is None:
    build_llvm(llvm_targets, jobs=num_jobs)
    llvm_config_path = llvm_build_dir / 'bin' / 'llvm-config'

build_bolt(
    c_path=c_path,
    cxx_path=cxx_path,
)

