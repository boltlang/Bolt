
declare const debug: (value: any) => void;

declare module NodeJS {
  interface Global {
    debug: (value: any) => void;
  }
}

