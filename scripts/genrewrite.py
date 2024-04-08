
import argparse
import sys

def main():

    parser = argparse.ArgumentParser()
    parser.add_argument('-o', '--out-file', required=True, help='The file to output generated code to')
    parser.add_argument('file', nargs=1, help='The specification file to load')
    args = parser.parse_args();

    print('DONE');

if __name__ == '__main__':
    sys.exit(main())

