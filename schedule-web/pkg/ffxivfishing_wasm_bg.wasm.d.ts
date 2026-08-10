/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const eorzea_time_from_unix: (a: bigint) => [number, number];
export const get_fish: (a: number) => [number, number, number, number];
export const get_fish_next_window: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
export const get_fish_windows: (a: number, b: bigint, c: number, d: number, e: number, f: number) => [number, number, number, number];
export const get_fish_windows_in_schedule: (a: number, b: bigint, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number, number, number];
export const get_weather_at_fish: (a: number, b: bigint) => [number, number, number, number];
export const init: (a: number, b: number) => [number, number];
export const init_default: () => [number, number];
export const list_all_fish: () => [number, number, number, number];
export const list_all_fish_info: () => [number, number, number, number];
export const search_fish: (a: number, b: number) => [number, number, number, number];
export const unix_from_eorzea_time: (a: bigint) => bigint;
export const unix_to_eorzea_esec: (a: bigint) => bigint;
export const __wbindgen_externrefs: WebAssembly.Table;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __externref_table_dealloc: (a: number) => void;
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_start: () => void;
