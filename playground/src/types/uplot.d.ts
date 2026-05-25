// Minimal ambient declarations for the uPlot ESM bundle copied from
// `node_modules/uplot/dist/uPlot.esm.js` into `_build/`. Covers only the
// surface used by `app.ts` — extend when adding new uPlot APIs.

declare module "./uPlot.esm.js" {
  export interface UplotScale {
    time?: boolean;
    range?: [number, number];
  }

  export interface UplotSeries {
    stroke?: string;
    width?: number;
    paths?: unknown;
  }

  export interface UplotOptions {
    width: number;
    height: number;
    pxAlign?: boolean;
    cursor?: { show: boolean };
    select?: { show: boolean };
    legend?: { show: boolean };
    scales?: { x?: UplotScale; y?: UplotScale };
    axes?: Array<{ show: boolean }>;
    series: UplotSeries[];
  }

  export type UplotData = [number[], number[]];

  export interface UplotPaths {
    stepped(opts: { align: 1 | -1 }): unknown;
  }

  export default class uPlot {
    constructor(opts: UplotOptions, data: UplotData, target: HTMLElement);
    static paths: UplotPaths;
  }
}
