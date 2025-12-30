// TypeScript services for resources commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface ResourceSnapshot {
    timestamp: number;
    cpu_percent: number;
    ram_used_bytes: number;
    ram_total_bytes: number;
    net_rx_bytes: number;
    net_tx_bytes: number;
}

export interface ResourceHistory {
    snapshots: ResourceSnapshot[];
    net_rx_speed: number[];
    net_tx_speed: number[];
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
