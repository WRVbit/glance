// TypeScript services for Tauri commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface SystemInfo {
    hostname: string;
    os_name: string;
    os_version: string;
    kernel_version: string;
    uptime_seconds: number;
    cpu_model: string;
    cpu_cores: number;
    cpu_threads: number;
}

export interface CpuStats {
    usage_percent: number;
    per_core: number[];
    frequency_mhz: number;
    core_count: number;
}

export interface MemoryStats {
    total_bytes: number;
    used_bytes: number;
    available_bytes: number;
    cached_bytes: number;
    swap_total_bytes: number;
    swap_used_bytes: number;
    usage_percent: number;
}

export interface DiskStats {
    name: string;
    mount_point: string;
    filesystem: string;
    total_bytes: number;
    used_bytes: number;
    available_bytes: number;
    usage_percent: number;
    is_removable: boolean;
}

export interface NetworkStats {
    interface: string;
    rx_bytes: number;
    tx_bytes: number;
    rx_packets: number;
    tx_packets: number;
}

export interface DistroInfo {
    id: string;
    name: string;
    version: string;
    version_codename: string;
    is_supported: boolean;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getSystemInfo(): Promise<SystemInfo> {
    return invoke('get_system_info');
}

export async function getCpuStats(): Promise<CpuStats> {
    return invoke('get_cpu_stats');
}

export async function getMemoryStats(): Promise<MemoryStats> {
    return invoke('get_memory_stats');
}

export async function getDiskStats(): Promise<DiskStats[]> {
    return invoke('get_disk_stats');
}

export async function getNetworkStats(): Promise<NetworkStats[]> {
    return invoke('get_network_stats');
}

export async function getDistroInfo(): Promise<DistroInfo> {
    return invoke('get_distro_info');
}
