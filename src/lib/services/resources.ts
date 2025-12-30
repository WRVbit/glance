// TypeScript services for enhanced resources monitoring
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface ResourceSnapshot {
    timestamp: number;
    cpu_percent: number;
    per_core_percent: number[];
    ram_used_bytes: number;
    ram_total_bytes: number;
    ram_cached_bytes: number;
    swap_used_bytes: number;
    swap_total_bytes: number;
    net_rx_bytes: number;
    net_tx_bytes: number;
    disk_read_bytes: number;
    disk_write_bytes: number;
}

export interface ResourceHistory {
    snapshots: ResourceSnapshot[];
    net_rx_speed: number[];
    net_tx_speed: number[];
    disk_read_speed: number[];
    disk_write_speed: number[];
    ram_history: number[];
}

export interface GpuInfo {
    name: string;
    vendor: string; // 'nvidia', 'amd', 'intel'
    vram_total_mb: number;
    vram_used_mb: number;
    usage_percent: number | null;
    temperature_c: number | null;
    driver_version: string | null;
}

export interface DiskIoStats {
    name: string;
    read_bytes: number;
    write_bytes: number;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getResourceSnapshot(): Promise<ResourceSnapshot> {
    return invoke('get_resource_snapshot');
}

export async function getResourceHistory(): Promise<ResourceHistory> {
    return invoke('get_resource_history');
}

export async function addResourceSnapshot(snapshot: ResourceSnapshot): Promise<void> {
    return invoke('add_resource_snapshot', { snapshot });
}

export async function clearResourceHistory(): Promise<void> {
    return invoke('clear_resource_history');
}

export async function getPerCoreUsage(): Promise<number[]> {
    return invoke('get_per_core_usage');
}

export async function getGpuInfo(): Promise<GpuInfo | null> {
    return invoke('get_gpu_info');
}

export async function getDiskIoStats(): Promise<DiskIoStats[]> {
    return invoke('get_disk_io_stats');
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Format bytes to human readable string with auto-scaling
 * @param bytes Number of bytes
 * @param decimals Decimal places
 * @returns Formatted string like "1.5 MB/s"
 */
export function formatSpeed(bytesPerSec: number, decimals = 1): string {
    if (bytesPerSec === 0) return '0 B/s';

    const k = 1024;
    const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    const i = Math.floor(Math.log(bytesPerSec) / Math.log(k));

    return parseFloat((bytesPerSec / Math.pow(k, i)).toFixed(decimals)) + ' ' + sizes[i];
}
