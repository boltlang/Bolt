export function parse(input:string):any;

export interface Location {
  line: number;
  column: number;
  offset: number;
}

interface LocationRange {
  start: Location,
  end: Location
}

export class SyntaxError {
  line: number;
  column: number;
  offset: number;
  location: LocationRange;
  expected:any[];
  found:any;
  name:string;
  message:string;
}

