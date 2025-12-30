// TypeScript services for packages commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface PackageInfo {
    name: string;
    version: string;
    size_bytes: number;
    description: string;
    is_auto: boolean;
    category: string;
}

export interface PackageAction {
    name: string;
    action: string;
    success: boolean;
    message: string;
}

export type PackageStats = [number, number, number]; // [total, auto, size]

// ============================================================================
// API Functions
// ============================================================================

export async function getPackages(): Promise<PackageInfo[]> {
    return invoke('get_packages');
}

export async function searchPackages(query: string): Promise<PackageInfo[]> {
    return invoke('search_packages', { query });
}

export async function uninstallPackage(name: string): Promise<PackageAction> {
    return invoke('uninstall_package', { name });
}

export async function purgePackage(name: string): Promise<PackageAction> {
    return invoke('purge_package', { name });
}

export async function autoremovePackages(): Promise<PackageAction> {
    return invoke('autoremove_packages');
}

export async function getPackageStats(): Promise<PackageStats> {
    return invoke('get_package_stats');
}
