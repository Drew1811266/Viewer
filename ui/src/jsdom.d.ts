declare module 'jsdom' {
  interface ConstructorOptions {
    beforeParse?(window: Window & typeof globalThis): void
    runScripts?: 'dangerously'
    url?: string
  }

  export class JSDOM {
    constructor(html?: string, options?: ConstructorOptions)
    readonly window: Window & typeof globalThis
  }
}
