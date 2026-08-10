/* tslint:disable */
/* eslint-disable */

export function eorzea_time_from_unix(unix_secs: bigint): string;

export function get_fish(fish_id: number): string;

export function get_fish_next_window(fish_id: number, timestamp_esec: bigint, filter_intuition: boolean, use_fish_eyes: boolean): string;

export function get_fish_windows(fish_id: number, timestamp_esec: bigint, limit: number, filter_intuition: boolean, use_fish_eyes: boolean, include_ongoing: boolean): string;

export function get_fish_windows_in_schedule(fish_id: number, timestamp_esec: bigint, schedule_json: string, timeperiod_secs: bigint, limit: number, timezone_name: string, filter_intuition: boolean, use_fish_eyes: boolean, include_ongoing: boolean): string;

export function get_weather_at_fish(fish_id: number, timestamp_esec: bigint): string;

export function init(data_json: string): void;

export function init_default(): void;

export function list_all_fish(): string;

export function list_all_fish_info(): string;

export function search_fish(query: string): string;

export function unix_from_eorzea_time(esec: bigint): bigint;

export function unix_to_eorzea_esec(unix_secs: bigint): bigint;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly eorzea_time_from_unix: (a: bigint) => [number, number];
    readonly get_fish: (a: number) => [number, number, number, number];
    readonly get_fish_next_window: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly get_fish_windows: (a: number, b: bigint, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly get_fish_windows_in_schedule: (a: number, b: bigint, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number, number, number];
    readonly get_weather_at_fish: (a: number, b: bigint) => [number, number, number, number];
    readonly init: (a: number, b: number) => [number, number];
    readonly init_default: () => [number, number];
    readonly list_all_fish: () => [number, number, number, number];
    readonly list_all_fish_info: () => [number, number, number, number];
    readonly search_fish: (a: number, b: number) => [number, number, number, number];
    readonly unix_from_eorzea_time: (a: bigint) => bigint;
    readonly unix_to_eorzea_esec: (a: bigint) => bigint;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
