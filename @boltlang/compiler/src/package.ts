
import * as path from "path"
import * as fs from "fs"

import yaml from "js-yaml"
import semver from "semver"

import {DiagnosticPrinter, E_FIELD_MUST_BE_BOOLEAN, E_FIELD_NOT_PRESENT, E_FIELD_MUST_BE_STRING, E_FIELD_HAS_INVALID_VERSION_NUMBER} from "./diagnostics";
import {hasOwnProperty, FastStringMap} from "./util";
import {isString} from "util"
import { SourceFile } from "./ast";

let nextPackageId = 1;

export class Package {

  public id = nextPackageId++;

  private sourceFilesByPath = new FastStringMap<string, SourceFile>();

  constructor(
    public rootDir: string,
    public name: string | null,
    public version: string | null,
    sourceFiles: SourceFile[],
    public isAutoImported: boolean,
    public isDependency: boolean,
  ) {
    for (const sourceFile of sourceFiles) {
      this.addSourceFile(sourceFile);
    }
  }

  public getAllSourceFiles(): IterableIterator<SourceFile> {
    return this.sourceFilesByPath.values();
  }

  public getMainLibrarySourceFile(): SourceFile | null {
    const fullPath = path.resolve(this.rootDir, 'lib.bolt');
    if (!this.sourceFilesByPath.has(fullPath)) {
      return null;
    }
    return this.sourceFilesByPath.get(fullPath)
  }

  public addSourceFile(sourceFile: SourceFile) {
    this.sourceFilesByPath.set(sourceFile.span!.file.fullPath, sourceFile);
  }

}

export function loadPackageMetadata(diagnostics: DiagnosticPrinter, filepath: string) {

  let name = null
  let version = null;
  let autoImport = false;

  let hasVersionErrors = false;
  let hasNameErrors = false;

  if (fs.existsSync(filepath)) {
    const data = yaml.safeLoad(fs.readFileSync(filepath, 'utf8'));
    if (data !== undefined) {
      if (hasOwnProperty(data, 'name')) {
        if (!isString(data.name)) {
          diagnostics.add({
            message: E_FIELD_MUST_BE_STRING,
            severity: 'error',
            args: { name: 'name' },
          });
          hasNameErrors = true;
        } else {
          name = data.name;
        }
      }
      if (hasOwnProperty(data, 'version')) {
        if (!isString(data.version)) {
          diagnostics.add({
            message: E_FIELD_MUST_BE_STRING,
            args: { name: 'version' },
            severity: 'error',
          });
          hasVersionErrors = true;
        } else {
          if (!semver.valid(data.version)) {
            diagnostics.add({
              message: E_FIELD_HAS_INVALID_VERSION_NUMBER,
              args: { name: 'version' },
              severity: 'error',
            });
            hasVersionErrors = true;
          } else {
            version = data.version;
          }
        }
      }
      if (hasOwnProperty(data, 'auto-import')) {
        if (typeof(data['auto-import']) !== 'boolean') {
          diagnostics.add({
            message: E_FIELD_MUST_BE_BOOLEAN,
            args: { name: 'auto-import' },
            severity: 'error', 
          })
        } else {
          autoImport = data['auto-import'];
        }
      }
    }
  }

  if (name === null && !hasNameErrors) {
    diagnostics.add({
      message: E_FIELD_NOT_PRESENT,
      severity: 'warning',
      args: { name: 'name' },
    });
  }

  if (version === null && !hasVersionErrors) {
    diagnostics.add({
      message: E_FIELD_NOT_PRESENT,
      severity: 'warning',
      args: { name: 'version' },
    });
  }

  return {
    name,
    version,
    autoImport,
  };

}
