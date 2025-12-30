// TypeScript services for cleaner commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface CleanupCategory {
    id: string;
    name: string;
    icon: string;
    size_bytes: number;
    file_count: number;
    requires_root: boolean;
    description: string;
}

export interface CleanupResult {
    category: string;
    success: boolean;
    bytes_freed: number;
    files_removed: number;
    message: string;
}

export interface ScheduleConfig {
    enabled: boolean;
    interval: string; // "daily", "weekly", "monthly"
    categories: string[];
    last_run: string | null;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getCleanupCategories(): Promise<CleanupCategory[]> {
    return invoke('get_cleanup_categories');
}

export async function cleanCategory(categoryId: string): Promise<CleanupResult> {
    return invoke('clean_category', { categoryId });
}

export async function getTotalReclaimable(): Promise<number> {
    return invoke('get_total_reclaimable');
}

// Auto-clean schedule functions
export async function getAutocleanSchedule(): Promise<ScheduleConfig> {
    return invoke('get_autoclean_schedule');
}

export async function setAutocleanSchedule(config: ScheduleConfig): Promise<string> {
    return invoke('set_autoclean_schedule', { config });
}

export async function getAutocleanStatus(): Promise<string> {
    return invoke('get_autoclean_status');
}

export async function runAutocleanNow(): Promise<string> {
    return invoke('run_autoclean_now');
}
