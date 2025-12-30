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
