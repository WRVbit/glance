<script>
  import { onMount } from "svelte";
  import * as systemService from "./lib/services/system";
  import * as cleanerService from "./lib/services/cleaner";
  import * as tweaksService from "./lib/services/tweaks";
  import * as servicesService from "./lib/services/services";
  import * as packagesService from "./lib/services/packages";
  import * as reposService from "./lib/services/repositories";
  import * as resourcesService from "./lib/services/resources";
  import * as hostsService from "./lib/services/hosts";
  import { invoke } from "@tauri-apps/api/core";

  // ============================================================================
  // State (Svelte 5 Runes)
  // ============================================================================

  /** @type {'dashboard' | 'cleaner' | 'tweaks' | 'services' | 'startup' | 'packages' | 'processes' | 'repositories' | 'resources' | 'hosts'} */
  let currentPage = $state("dashboard");
  let loading = $state(true);

  // System stats
  let systemInfo = $state(null);
  let cpuStats = $state(null);
  let memoryStats = $state(null);
  let diskStats = $state([]);
  let distroInfo = $state(null);

  // Cleaner
  let cleanupCategories = $state([]);
  let totalReclaimable = $state(0);
  let cleaningCategory = $state(null);

  // Tweaks
  let tweakCategories = $state([]);
  let applyingTweak = $state(null);

  // Services
  let services = $state([]);
  let servicesFilter = $state("");
  let loadingServices = $state(false);

  // Packages
  let packages = $state([]);
  let packagesFilter = $state("");
  let packageStats = $state([0, 0, 0]);
  let loadingPackages = $state(false);

  // Processes
  let processes = $state([]);
  let processesFilter = $state("");
  let loadingProcesses = $state(false);

  // Startup
  let startupApps = $state([]);
  let loadingStartup = $state(false);

  // Repositories
  let repositories = $state([]);
  let mirrors = $state([]);
  let loadingRepos = $state(false);
  let testingMirrors = $state(false);
  let newPpa = $state("");

  // Resources (history for graphs)
  let resourceHistory = $state({
    snapshots: [],
    net_rx_speed: [],
    net_tx_speed: [],
  });
  let resourceInterval = $state(null);

  // Hosts
  let hostEntries = $state([]);
  let hostsStats = $state({
    total_entries: 0,
    enabled_entries: 0,
    blocked_domains: 0,
  });
  let blocklists = $state([]);
  let loadingHosts = $state(false);
  let newHostIp = $state("0.0.0.0");
  let newHostname = $state("");

  // Refresh interval
  let refreshInterval = $state(null);

  // ============================================================================
  // Navigation
  // ============================================================================

  const navItems = [
    { id: "dashboard", icon: "📊", label: "Dashboard" },
    { id: "resources", icon: "📈", label: "Resources" },
    { id: "cleaner", icon: "🧹", label: "System Cleaner" },
    { id: "tweaks", icon: "⚙️", label: "Performance Tweaks" },
    { id: "repositories", icon: "📦", label: "Repositories" },
    { id: "services", icon: "🔧", label: "Services" },
    { id: "startup", icon: "🚀", label: "Startup Apps" },
    { id: "packages", icon: "📥", label: "Packages" },
    { id: "processes", icon: "⚡", label: "Processes" },
    { id: "hosts", icon: "🌐", label: "Hosts Editor" },
  ];

  // ============================================================================
  // Utilities
  // ============================================================================

  function formatBytes(bytes, decimals = 1) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return (
      parseFloat((bytes / Math.pow(k, i)).toFixed(decimals)) + " " + sizes[i]
    );
  }

  function formatUptime(seconds) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m`;
  }

  // ============================================================================
  // Data Loading
  // ============================================================================

  async function loadDashboard() {
    try {
      [systemInfo, cpuStats, memoryStats, diskStats, distroInfo] =
        await Promise.all([
          systemService.getSystemInfo(),
          systemService.getCpuStats(),
          systemService.getMemoryStats(),
          systemService.getDiskStats(),
          systemService.getDistroInfo(),
        ]);
    } catch (e) {
      console.error("Failed to load dashboard:", e);
    }
  }

  async function refreshStats() {
    if (currentPage !== "dashboard") return;
    try {
      [cpuStats, memoryStats] = await Promise.all([
        systemService.getCpuStats(),
        systemService.getMemoryStats(),
      ]);
    } catch (e) {
      console.error("Failed to refresh stats:", e);
    }
  }

  async function loadCleaner() {
    try {
      [cleanupCategories, totalReclaimable] = await Promise.all([
        cleanerService.getCleanupCategories(),
        cleanerService.getTotalReclaimable(),
      ]);
    } catch (e) {
      console.error("Failed to load cleaner:", e);
    }
  }

  async function loadTweaks() {
    try {
      tweakCategories = await tweaksService.getTweaks();
    } catch (e) {
      console.error("Failed to load tweaks:", e);
    }
  }

  async function loadServices() {
    loadingServices = true;
    try {
      if (servicesFilter) {
        services = await servicesService.searchServices(servicesFilter);
      } else {
        services = await servicesService.getServices();
      }
    } catch (e) {
      console.error("Failed to load services:", e);
    }
    loadingServices = false;
  }

  async function loadPackages() {
    loadingPackages = true;
    try {
      if (packagesFilter) {
        packages = await packagesService.searchPackages(packagesFilter);
      } else {
        packages = await packagesService.getPackages();
      }
      packageStats = await packagesService.getPackageStats();
    } catch (e) {
      console.error("Failed to load packages:", e);
    }
    loadingPackages = false;
  }

  async function loadProcesses() {
    loadingProcesses = true;
    try {
      processes = await invoke("get_processes");
      if (processesFilter) {
        const filter = processesFilter.toLowerCase();
        processes = processes.filter(
          (p) =>
            p.name.toLowerCase().includes(filter) ||
            p.command.toLowerCase().includes(filter),
        );
      }
    } catch (e) {
      console.error("Failed to load processes:", e);
    }
    loadingProcesses = false;
  }

  async function loadStartup() {
    loadingStartup = true;
    try {
      startupApps = await invoke("get_startup_apps");
    } catch (e) {
      console.error("Failed to load startup apps:", e);
    }
    loadingStartup = false;
  }

  async function loadRepositories() {
    loadingRepos = true;
    try {
      [repositories, mirrors] = await Promise.all([
        reposService.getRepositories(),
        reposService.getMirrors(),
      ]);
    } catch (e) {
      console.error("Failed to load repositories:", e);
    }
    loadingRepos = false;
  }

  async function loadResources() {
    try {
      const snapshot = await resourcesService.getResourceSnapshot();
      await resourcesService.addResourceSnapshot(snapshot);
      resourceHistory = await resourcesService.getResourceHistory();
    } catch (e) {
      console.error("Failed to load resources:", e);
    }
  }

  async function loadHosts() {
    loadingHosts = true;
    try {
      [hostEntries, hostsStats, blocklists] = await Promise.all([
        hostsService.getHosts(),
        hostsService.getHostsStats(),
        hostsService.getBlocklists(),
      ]);
    } catch (e) {
      console.error("Failed to load hosts:", e);
    }
    loadingHosts = false;
  }

  // ============================================================================
  // Actions
  // ============================================================================

  async function handleClean(categoryId) {
    cleaningCategory = categoryId;
    try {
      await cleanerService.cleanCategory(categoryId);
      await loadCleaner();
    } catch (e) {
      console.error("Failed to clean:", e);
    }
    cleaningCategory = null;
  }

  async function handleApplyTweak(tweakId, value) {
    applyingTweak = tweakId;
    try {
      await tweaksService.applyTweak(tweakId, value);
      await loadTweaks();
    } catch (e) {
      console.error("Failed to apply tweak:", e);
    }
    applyingTweak = null;
  }

  async function handleApplyAllTweaks() {
    applyingTweak = "all";
    try {
      await tweaksService.applyAllRecommended();
      await loadTweaks();
    } catch (e) {
      console.error("Failed to apply tweaks:", e);
    }
    applyingTweak = null;
  }

  async function handleServiceAction(name, action) {
    try {
      if (action === "start") await servicesService.startService(name);
      else if (action === "stop") await servicesService.stopService(name);
      else if (action === "restart") await servicesService.restartService(name);
      else if (action === "enable") await servicesService.enableService(name);
      else if (action === "disable") await servicesService.disableService(name);
      await loadServices();
    } catch (e) {
      console.error("Service action failed:", e);
    }
  }

  async function handleKillProcess(pid) {
    try {
      await invoke("kill_process", { pid });
      await loadProcesses();
    } catch (e) {
      console.error("Failed to kill process:", e);
    }
  }

  async function handleToggleStartup(app) {
    try {
      if (app.is_enabled) {
        await invoke("disable_startup_app", { filePath: app.file_path });
      } else {
        await invoke("enable_startup_app", { filePath: app.file_path });
      }
      await loadStartup();
    } catch (e) {
      console.error("Failed to toggle startup app:", e);
    }
  }

  async function handleAddPpa() {
    if (!newPpa.trim()) return;
    try {
      await reposService.addPpa(newPpa.trim());
      newPpa = "";
      await loadRepositories();
    } catch (e) {
      console.error("Failed to add PPA:", e);
    }
  }

  async function handleTestMirrors() {
    testingMirrors = true;
    try {
      mirrors = await reposService.testAllMirrors();
    } catch (e) {
      console.error("Failed to test mirrors:", e);
    }
    testingMirrors = false;
  }

  async function handleSetMirror(uri) {
    try {
      await reposService.setMirror(uri);
      await reposService.aptUpdate();
      await loadRepositories();
    } catch (e) {
      console.error("Failed to set mirror:", e);
    }
  }

  async function handleAddHost() {
    if (!newHostname.trim()) return;
    try {
      await hostsService.addHost(newHostIp, newHostname.trim());
      newHostname = "";
      await loadHosts();
    } catch (e) {
      console.error("Failed to add host:", e);
    }
  }

  async function handleToggleHost(lineNumber) {
    try {
      await hostsService.toggleHost(lineNumber);
      await loadHosts();
    } catch (e) {
      console.error("Failed to toggle host:", e);
    }
  }

  async function handleRemoveHost(lineNumber) {
    try {
      await hostsService.removeHost(lineNumber);
      await loadHosts();
    } catch (e) {
      console.error("Failed to remove host:", e);
    }
  }

  async function handleImportBlocklist(url) {
    try {
      const count = await hostsService.importBlocklist(url);
      console.log(`Imported ${count} entries`);
      await loadHosts();
    } catch (e) {
      console.error("Failed to import blocklist:", e);
    }
  }

  // ============================================================================
  // Lifecycle
  // ============================================================================

  onMount(async () => {
    await loadDashboard();
    loading = false;

    // Start refresh interval for stats
    refreshInterval = setInterval(refreshStats, 2000);

    return () => {
      if (refreshInterval) clearInterval(refreshInterval);
    };
  });

  // Load page-specific data when page changes
  $effect(() => {
    // Cleanup function - MANDATORY to prevent interval stacking
    let intervalId = null;

    if (currentPage === "cleaner") loadCleaner();
    else if (currentPage === "tweaks") loadTweaks();
    else if (currentPage === "services") loadServices();
    else if (currentPage === "packages") loadPackages();
    else if (currentPage === "processes") loadProcesses();
    else if (currentPage === "startup") loadStartup();
    else if (currentPage === "repositories") loadRepositories();
    else if (currentPage === "hosts") loadHosts();
    else if (currentPage === "resources") {
      loadResources();
      // Start resource polling with local intervalId
      intervalId = setInterval(loadResources, 1000);
      resourceInterval = intervalId;
    }

    // Return cleanup function - this runs when currentPage changes or component unmounts
    return () => {
      if (intervalId) {
        clearInterval(intervalId);
      }
      if (resourceInterval) {
        clearInterval(resourceInterval);
        resourceInterval = null;
      }
    };
  });

  // Derived values
  let cpuPercent = $derived(cpuStats?.usage_percent ?? 0);
  let memPercent = $derived(memoryStats?.usage_percent ?? 0);
  let mainDisk = $derived(
    diskStats.find((d) => d.mount_point === "/") || diskStats[0],
  );
  let diskPercent = $derived(mainDisk?.usage_percent ?? 0);
  let filteredServices = $derived(services.slice(0, 100)); // Limit for perf
  let filteredPackages = $derived(packages.slice(0, 100));
  let filteredProcesses = $derived(processes.slice(0, 100));
</script>

<div class="flex h-screen bg-surface-900 overflow-hidden">
  <!-- Background gradient -->
  <div class="fixed inset-0 pointer-events-none">
    <div
      class="absolute top-0 left-0 w-96 h-96 bg-primary-600/20 rounded-full blur-3xl"
    ></div>
    <div
      class="absolute bottom-0 right-0 w-96 h-96 bg-accent-500/10 rounded-full blur-3xl"
    ></div>
  </div>

  <!-- Sidebar -->
  <aside
    class="relative w-64 glass border-r border-white/10 flex flex-col p-4 z-10"
  >
    <div class="flex items-center gap-3 px-4 py-4 mb-6">
      <div
        class="w-10 h-10 rounded-xl bg-gradient-to-br from-primary-500 to-accent-500 flex items-center justify-center text-xl"
      >
        🚀
      </div>
      <div>
        <h1 class="font-bold text-lg text-gradient">Glance</h1>
        <p class="text-xs text-gray-500">v0.1.0</p>
      </div>
    </div>

    <nav class="flex-1 space-y-1">
      {#each navItems as item}
        <button
          class="nav-item w-full"
          class:active={currentPage === item.id}
          onclick={() => (currentPage = item.id)}
        >
          <span class="text-xl">{item.icon}</span>
          <span>{item.label}</span>
        </button>
      {/each}
    </nav>

    {#if distroInfo}
      <div class="mt-4 p-4 glass rounded-xl text-xs">
        <p class="text-gray-400">{distroInfo.name}</p>
        <p class="text-gray-500">{distroInfo.version}</p>
      </div>
    {/if}
  </aside>

  <!-- Main Content -->
  <main class="flex-1 flex flex-col overflow-hidden relative z-10">
    <!-- Header -->
    <header class="glass border-b border-white/10 px-8 py-6">
      <h2 class="text-2xl font-bold">
        {navItems.find((n) => n.id === currentPage)?.label}
      </h2>
    </header>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-8">
      {#if loading}
        <div class="flex items-center justify-center h-full">
          <div class="spinner w-10 h-10"></div>
        </div>
      {:else if currentPage === "dashboard"}
        <!-- Dashboard -->
        <div class="space-y-6">
          <!-- System Info -->
          {#if systemInfo}
            <div class="card">
              <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-semibold">System Information</h3>
                <span class="text-2xl">💻</span>
              </div>
              <div class="grid grid-cols-2 md:grid-cols-3 gap-4 text-sm">
                <div>
                  <span class="text-gray-500">Hostname:</span>
                  <span class="ml-2">{systemInfo.hostname}</span>
                </div>
                <div>
                  <span class="text-gray-500">OS:</span>
                  <span class="ml-2">{systemInfo.os_name}</span>
                </div>
                <div>
                  <span class="text-gray-500">Version:</span>
                  <span class="ml-2">{systemInfo.os_version}</span>
                </div>
                <div>
                  <span class="text-gray-500">Kernel:</span>
                  <span class="ml-2">{systemInfo.kernel_version}</span>
                </div>
                <div>
                  <span class="text-gray-500">Uptime:</span>
                  <span class="ml-2"
                    >{formatUptime(systemInfo.uptime_seconds)}</span
                  >
                </div>
                <div>
                  <span class="text-gray-500">CPU:</span>
                  <span class="ml-2"
                    >{systemInfo.cpu_cores} cores / {systemInfo.cpu_threads} threads</span
                  >
                </div>
              </div>
            </div>
          {/if}

          <!-- Gauges -->
          <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
            <!-- CPU -->
            <div class="card card-hover">
              <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">CPU</h3>
                <span class="text-xl">⚡</span>
              </div>
              <div class="gauge-container">
                <div class="relative w-32 h-32">
                  <svg class="w-full h-full -rotate-90" viewBox="0 0 100 100">
                    <circle
                      cx="50"
                      cy="50"
                      r="45"
                      stroke-width="8"
                      class="fill-none stroke-surface-700"
                    />
                    <circle
                      cx="50"
                      cy="50"
                      r="45"
                      stroke-width="8"
                      class="fill-none stroke-primary-500"
                      stroke-linecap="round"
                      stroke-dasharray={2 * Math.PI * 45}
                      stroke-dashoffset={2 *
                        Math.PI *
                        45 *
                        (1 - cpuPercent / 100)}
                      style="transition: stroke-dashoffset 0.5s ease"
                    />
                  </svg>
                  <div
                    class="absolute inset-0 flex flex-col items-center justify-center"
                  >
                    <span class="text-2xl font-bold text-gradient"
                      >{cpuPercent.toFixed(0)}%</span
                    >
                    <span class="text-xs text-gray-500">CPU</span>
                  </div>
                </div>
                {#if cpuStats}
                  <p class="text-sm text-gray-400">
                    {cpuStats.frequency_mhz} MHz
                  </p>
                {/if}
              </div>
            </div>

            <!-- Memory -->
            <div class="card card-hover">
              <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">Memory</h3>
                <span class="text-xl">🧠</span>
              </div>
              <div class="gauge-container">
                <div class="relative w-32 h-32">
                  <svg class="w-full h-full -rotate-90" viewBox="0 0 100 100">
                    <circle
                      cx="50"
                      cy="50"
                      r="45"
                      stroke-width="8"
                      class="fill-none stroke-surface-700"
                    />
                    <circle
                      cx="50"
                      cy="50"
                      r="45"
                      stroke-width="8"
                      class="fill-none stroke-accent-500"
                      stroke-linecap="round"
                      stroke-dasharray={2 * Math.PI * 45}
                      stroke-dashoffset={2 *
                        Math.PI *
                        45 *
                        (1 - memPercent / 100)}
                      style="transition: stroke-dashoffset 0.5s ease"
                    />
                  </svg>
                  <div
                    class="absolute inset-0 flex flex-col items-center justify-center"
                  >
                    <span class="text-2xl font-bold text-gradient"
                      >{memPercent.toFixed(0)}%</span
                    >
                    <span class="text-xs text-gray-500">RAM</span>
                  </div>
                </div>
                {#if memoryStats}
                  <p class="text-sm text-gray-400">
                    {formatBytes(memoryStats.used_bytes)} / {formatBytes(
                      memoryStats.total_bytes,
                    )}
                  </p>
                {/if}
              </div>
            </div>

            <!-- Disk -->
            <div class="card card-hover">
              <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">Disk</h3>
                <span class="text-xl">💾</span>
              </div>
              <div class="gauge-container">
                <div class="relative w-32 h-32">
                  <svg class="w-full h-full -rotate-90" viewBox="0 0 100 100">
                    <circle
                      cx="50"
                      cy="50"
                      r="45"
                      stroke-width="8"
                      class="fill-none stroke-surface-700"
                    />
                    <circle
                      cx="50"
                      cy="50"
                      r="45"
                      stroke-width="8"
                      class="fill-none stroke-emerald-500"
                      stroke-linecap="round"
                      stroke-dasharray={2 * Math.PI * 45}
                      stroke-dashoffset={2 *
                        Math.PI *
                        45 *
                        (1 - diskPercent / 100)}
                      style="transition: stroke-dashoffset 0.5s ease"
                    />
                  </svg>
                  <div
                    class="absolute inset-0 flex flex-col items-center justify-center"
                  >
                    <span class="text-2xl font-bold text-gradient"
                      >{diskPercent.toFixed(0)}%</span
                    >
                    <span class="text-xs text-gray-500">Disk</span>
                  </div>
                </div>
                {#if mainDisk}
                  <p class="text-sm text-gray-400">
                    {formatBytes(mainDisk.used_bytes)} / {formatBytes(
                      mainDisk.total_bytes,
                    )}
                  </p>
                {/if}
              </div>
            </div>
          </div>

          <!-- Disk List -->
          {#if diskStats.length > 0}
            <div class="card">
              <h3 class="font-semibold mb-4">Storage Devices</h3>
              <div class="space-y-3">
                {#each diskStats as disk}
                  <div class="list-item">
                    <div class="flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium">{disk.mount_point}</span>
                        <span class="badge badge-info">{disk.filesystem}</span>
                      </div>
                      <div class="mt-2">
                        <div class="progress">
                          <div
                            class="progress-bar"
                            style="width: {disk.usage_percent}%"
                          ></div>
                        </div>
                      </div>
                    </div>
                    <div class="text-right text-sm">
                      <p>{formatBytes(disk.used_bytes)}</p>
                      <p class="text-gray-500">
                        of {formatBytes(disk.total_bytes)}
                      </p>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {:else if currentPage === "cleaner"}
        <!-- Cleaner -->
        <div class="space-y-6">
          <div class="card text-center py-8">
            <p class="text-5xl font-bold text-gradient">
              {formatBytes(totalReclaimable)}
            </p>
            <p class="text-gray-400 mt-2">Total reclaimable space</p>
          </div>

          <div class="space-y-3">
            {#each cleanupCategories as category}
              <div class="list-item">
                <div class="flex items-center gap-4">
                  <span class="text-3xl">{category.icon}</span>
                  <div>
                    <h4 class="font-medium">{category.name}</h4>
                    <p class="text-sm text-gray-500">{category.description}</p>
                    <p class="text-sm text-gradient">
                      {formatBytes(category.size_bytes)}
                    </p>
                  </div>
                </div>
                <button
                  class="btn btn-primary btn-sm"
                  disabled={cleaningCategory === category.id ||
                    category.size_bytes === 0}
                  onclick={() => handleClean(category.id)}
                >
                  {#if cleaningCategory === category.id}
                    <span class="spinner"></span>
                  {:else}
                    Clean
                  {/if}
                </button>
              </div>
            {/each}
          </div>
        </div>
      {:else if currentPage === "tweaks"}
        <!-- Tweaks -->
        <div class="space-y-6">
          <div class="flex justify-end">
            <button
              class="btn btn-primary"
              disabled={applyingTweak === "all"}
              onclick={handleApplyAllTweaks}
            >
              {#if applyingTweak === "all"}
                <span class="spinner"></span>
              {:else}
                Apply All Recommended
              {/if}
            </button>
          </div>

          {#each tweakCategories as category}
            <div class="card">
              <div class="flex items-center gap-2 mb-4">
                <span class="text-xl">{category.icon}</span>
                <h3 class="font-semibold">{category.name}</h3>
              </div>
              <div class="space-y-3">
                {#each category.tweaks as tweak}
                  <div
                    class="list-item"
                    class:border-emerald-500={tweak.is_applied}
                  >
                    <div class="flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium">{tweak.name}</span>
                        {#if tweak.is_applied}
                          <span class="badge badge-success">Applied</span>
                        {/if}
                      </div>
                      <p class="text-sm text-gray-500 mt-1">
                        {tweak.description}
                      </p>
                      <div class="flex gap-4 mt-2 text-xs">
                        <span
                          >Current: <span class="text-gray-300"
                            >{tweak.current_value}</span
                          ></span
                        >
                        <span
                          >Recommended: <span class="text-accent-400"
                            >{tweak.recommended_value}</span
                          ></span
                        >
                      </div>
                    </div>
                    {#if !tweak.is_applied}
                      <button
                        class="btn btn-primary btn-sm"
                        disabled={applyingTweak === tweak.id}
                        onclick={() =>
                          handleApplyTweak(tweak.id, tweak.recommended_value)}
                      >
                        {#if applyingTweak === tweak.id}
                          <span class="spinner"></span>
                        {:else}
                          Apply
                        {/if}
                      </button>
                    {:else}
                      <span class="text-emerald-400">✓</span>
                    {/if}
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      {:else if currentPage === "services"}
        <!-- Services -->
        <div class="space-y-6">
          <div class="flex gap-4">
            <input
              type="text"
              class="input flex-1"
              placeholder="Search services..."
              bind:value={servicesFilter}
              oninput={() => loadServices()}
            />
          </div>

          {#if loadingServices}
            <div class="flex justify-center py-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            <div class="text-sm text-gray-500 mb-2">
              Showing {filteredServices.length} of {services.length} services
            </div>
            <div class="space-y-2">
              {#each filteredServices as service}
                <div class="list-item">
                  <div class="flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-medium">{service.name}</span>
                      {#if service.active_state === "active"}
                        <span class="badge badge-success">Active</span>
                      {:else}
                        <span class="badge badge-danger">Inactive</span>
                      {/if}
                      {#if service.is_enabled}
                        <span class="badge badge-info">Enabled</span>
                      {/if}
                    </div>
                    <p class="text-sm text-gray-500 truncate">
                      {service.description}
                    </p>
                  </div>
                  <div class="flex gap-2">
                    {#if service.active_state === "active"}
                      <button
                        class="btn btn-secondary btn-sm"
                        onclick={() =>
                          handleServiceAction(service.name, "stop")}
                        >Stop</button
                      >
                      <button
                        class="btn btn-secondary btn-sm"
                        onclick={() =>
                          handleServiceAction(service.name, "restart")}
                        >Restart</button
                      >
                    {:else}
                      <button
                        class="btn btn-success btn-sm"
                        onclick={() =>
                          handleServiceAction(service.name, "start")}
                        >Start</button
                      >
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if currentPage === "startup"}
        <!-- Startup -->
        <div class="space-y-6">
          {#if loadingStartup}
            <div class="flex justify-center py-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            <div class="space-y-2">
              {#each startupApps as app}
                <div class="list-item">
                  <div class="flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-medium">{app.name}</span>
                      {#if app.is_system}
                        <span class="badge badge-warning">System</span>
                      {/if}
                    </div>
                    <p class="text-sm text-gray-500 truncate">{app.exec}</p>
                  </div>
                  <button
                    class="toggle"
                    class:bg-primary-600={app.is_enabled}
                    class:bg-surface-700={!app.is_enabled}
                    onclick={() => handleToggleStartup(app)}
                  >
                    <span
                      class="toggle-dot"
                      class:translate-x-5={app.is_enabled}
                      class:translate-x-1={!app.is_enabled}
                    ></span>
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if currentPage === "packages"}
        <!-- Packages -->
        <div class="space-y-6">
          <div class="grid grid-cols-3 gap-4">
            <div class="stat-card">
              <p class="stat-value">{packageStats[0]}</p>
              <p class="stat-label">Total Packages</p>
            </div>
            <div class="stat-card">
              <p class="stat-value">{packageStats[1]}</p>
              <p class="stat-label">Auto Installed</p>
            </div>
            <div class="stat-card">
              <p class="stat-value">{formatBytes(packageStats[2])}</p>
              <p class="stat-label">Total Size</p>
            </div>
          </div>

          <div class="flex gap-4">
            <input
              type="text"
              class="input flex-1"
              placeholder="Search packages..."
              bind:value={packagesFilter}
              oninput={() => loadPackages()}
            />
          </div>

          {#if loadingPackages}
            <div class="flex justify-center py-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            <div class="text-sm text-gray-500 mb-2">
              Showing {filteredPackages.length} of {packages.length} packages
            </div>
            <div class="space-y-2">
              {#each filteredPackages as pkg}
                <div class="list-item">
                  <div class="flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-medium">{pkg.name}</span>
                      {#if pkg.is_auto}
                        <span class="badge badge-info text-xs">Auto</span>
                      {/if}
                    </div>
                    <p class="text-sm text-gray-500 truncate">
                      {pkg.description}
                    </p>
                    <p class="text-xs text-gray-600">
                      {pkg.version} • {formatBytes(pkg.size_bytes)}
                    </p>
                  </div>
                  <button
                    class="btn btn-danger btn-sm"
                    onclick={() =>
                      packagesService
                        .uninstallPackage(pkg.name)
                        .then(loadPackages)}
                  >
                    Uninstall
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if currentPage === "processes"}
        <!-- Processes -->
        <div class="space-y-6">
          <div class="flex gap-4">
            <input
              type="text"
              class="input flex-1"
              placeholder="Search processes..."
              bind:value={processesFilter}
              oninput={() => loadProcesses()}
            />
            <button class="btn btn-secondary" onclick={loadProcesses}>
              Refresh
            </button>
          </div>

          {#if loadingProcesses}
            <div class="flex justify-center py-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            <div class="text-sm text-gray-500 mb-2">
              Showing {filteredProcesses.length} of {processes.length} processes
            </div>
            <div class="space-y-2">
              {#each filteredProcesses as proc}
                <div class="list-item">
                  <div class="flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-medium">{proc.name}</span>
                      <span class="badge badge-info">PID: {proc.pid}</span>
                    </div>
                    <div class="flex gap-4 text-sm text-gray-500 mt-1">
                      <span>CPU: {proc.cpu_usage.toFixed(1)}%</span>
                      <span>RAM: {formatBytes(proc.memory_bytes)}</span>
                      <span class="truncate max-w-xs">{proc.command}</span>
                    </div>
                  </div>
                  <button
                    class="btn btn-danger btn-sm"
                    onclick={() => handleKillProcess(proc.pid)}
                  >
                    Kill
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if currentPage === "repositories"}
        <!-- Repositories -->
        <div class="space-y-6">
          <div class="card">
            <h3 class="font-semibold mb-4">Manage Repositories</h3>

            <!-- PPA Input -->
            <div class="flex gap-4 mb-6">
              <input
                type="text"
                class="input flex-1"
                placeholder="Add PPA (e.g. ppa:user/repo)..."
                bind:value={newPpa}
              />
              <button class="btn btn-primary" onclick={handleAddPpa}
                >Add PPA</button
              >
            </div>

            <!-- Mirrors -->
            <div class="border-t border-white/10 pt-6">
              <div class="flex items-center justify-between mb-4">
                <h4 class="font-medium">Ubuntu Mirrors</h4>
                <button
                  class="btn btn-secondary btn-sm"
                  disabled={testingMirrors}
                  onclick={handleTestMirrors}
                >
                  {#if testingMirrors}
                    <span class="spinner w-4 h-4 mr-2"></span> Testing...
                  {:else}
                    Test Speeds
                  {/if}
                </button>
              </div>

              <div
                class="bg-surface-800 rounded-lg p-2 max-h-40 overflow-y-auto"
              >
                {#each mirrors as mirror}
                  <div
                    class="flex items-center justify-between p-2 hover:bg-surface-700 rounded cursor-pointer"
                    onclick={() => handleSetMirror(mirror.uri)}
                  >
                    <div class="flex flex-col">
                      <span class="text-sm font-medium"
                        >{mirror.name} ({mirror.country})</span
                      >
                      <span class="text-xs text-gray-500 truncate"
                        >{mirror.uri}</span
                      >
                    </div>
                    <div class="flex items-center gap-2">
                      {#if mirror.latency_ms !== null}
                        <span
                          class="text-xs font-mono {mirror.latency_ms < 100
                            ? 'text-green-400'
                            : 'text-yellow-400'}"
                        >
                          {mirror.latency_ms}ms
                        </span>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          </div>

          <!-- Repo List -->
          {#if loadingRepos}
            <div class="flex justify-center py-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            <div class="space-y-2">
              {#each repositories as repo}
                <div class="list-item">
                  <div class="flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-medium">{repo.repo_type}</span>
                      {#if repo.is_ppa}
                        <span class="badge badge-accent">PPA</span>
                      {/if}
                      <span class="text-sm text-gray-400">{repo.suite}</span>
                    </div>
                    <p class="text-xs text-gray-500 mt-1 truncate">
                      {repo.uri}
                    </p>
                    <p class="text-xs text-gray-600 truncate">
                      {repo.components.join(" ")}
                    </p>
                  </div>
                  <div class="flex items-center gap-2">
                    <button
                      class="toggle"
                      class:bg-primary-600={repo.is_enabled}
                      class:bg-surface-700={!repo.is_enabled}
                      onclick={() =>
                        invoke("toggle_repository", {
                          filePath: repo.file_path,
                          lineNumber: repo.line_number,
                        }).then(loadRepositories)}
                    >
                      <span
                        class="toggle-dot"
                        class:translate-x-5={repo.is_enabled}
                        class:translate-x-1={!repo.is_enabled}
                      ></span>
                    </button>
                    {#if repo.is_ppa}
                      <button
                        class="btn btn-danger btn-sm"
                        onclick={() =>
                          reposService
                            .removePpa(repo.uri)
                            .then(loadRepositories)}
                      >
                        ✕
                      </button>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if currentPage === "resources"}
        <!-- Resources -->
        <div class="space-y-6">
          <!-- CPU Chart -->
          <div class="card">
            <h3 class="font-semibold mb-4 flex items-center gap-2">
              <span class="text-xl">⚡</span> CPU History
            </h3>
            <div
              class="relative h-48 bg-surface-900/50 rounded-lg overflow-hidden flex items-end"
            >
              <!-- Grid lines -->
              <div
                class="absolute inset-x-0 bottom-0 h-full border-b border-surface-700"
              ></div>
              <div
                class="absolute inset-x-0 bottom-1/2 h-px bg-surface-700/50 dashed"
              ></div>

              <!-- Chart -->
              <svg
                class="w-full h-full"
                viewBox="0 0 100 100"
                preserveAspectRatio="none"
              >
                <polyline
                  vector-effect="non-scaling-stroke"
                  points={resourceHistory.snapshots
                    .map(
                      (s, i) =>
                        `${(i / Math.max(1, resourceHistory.snapshots.length - 1)) * 100},${100 - s.cpu_percent}`,
                    )
                    .join(" ")}
                  fill="none"
                  stroke="currentColor"
                  class="text-primary-500"
                  stroke-width="2"
                />
              </svg>
              <!-- Gradient fill under line (simulated) -->
              <div
                class="absolute inset-0 bg-gradient-to-t from-primary-500/20 to-transparent pointer-events-none"
              ></div>
            </div>
            <div class="flex justify-between mt-2 text-xs text-gray-500">
              <span>60s ago</span>
              <span>Now</span>
            </div>
          </div>

          <!-- Network Chart -->
          <div class="card">
            <h3 class="font-semibold mb-4 flex items-center gap-2">
              <span class="text-xl">🌐</span> Network Traffic
            </h3>
            <div
              class="relative h-48 bg-surface-900/50 rounded-lg overflow-hidden"
            >
              <svg
                class="w-full h-full"
                viewBox="0 0 100 100"
                preserveAspectRatio="none"
              >
                <!-- Download (Green) -->
                <polyline
                  vector-effect="non-scaling-stroke"
                  points={resourceHistory.net_rx_speed
                    .map((s, i) => {
                      const max = Math.max(
                        ...resourceHistory.net_rx_speed,
                        1024,
                      ); // Min 1KB scale
                      return `${(i / Math.max(1, resourceHistory.net_rx_speed.length - 1)) * 100},${100 - (s / max) * 90}`;
                    })
                    .join(" ")}
                  fill="none"
                  stroke="#10b981"
                  stroke-width="2"
                />
                <!-- Upload (Blue) -->
                <polyline
                  vector-effect="non-scaling-stroke"
                  points={resourceHistory.net_tx_speed
                    .map((s, i) => {
                      const max = Math.max(
                        ...resourceHistory.net_tx_speed,
                        1024,
                      );
                      return `${(i / Math.max(1, resourceHistory.net_tx_speed.length - 1)) * 100},${100 - (s / max) * 90}`;
                    })
                    .join(" ")}
                  fill="none"
                  stroke="#3b82f6"
                  stroke-width="2"
                />
              </svg>

              <div
                class="absolute top-2 right-2 text-xs text-right bg-surface-900/80 p-2 rounded"
              >
                <p class="text-emerald-400">
                  ⬇ {formatBytes(
                    resourceHistory.net_rx_speed[
                      resourceHistory.net_rx_speed.length - 1
                    ] || 0,
                  )}/s
                </p>
                <p class="text-blue-400">
                  ⬆ {formatBytes(
                    resourceHistory.net_tx_speed[
                      resourceHistory.net_tx_speed.length - 1
                    ] || 0,
                  )}/s
                </p>
              </div>
            </div>
          </div>
        </div>
      {:else if currentPage === "hosts"}
        <!-- Hosts Editor -->
        <div class="space-y-6">
          <div class="grid grid-cols-3 gap-4">
            <div class="stat-card">
              <p class="stat-value">{hostsStats.total_entries}</p>
              <p class="stat-label">Total Entries</p>
            </div>
            <div class="stat-card">
              <p class="stat-value">{hostsStats.enabled_entries}</p>
              <p class="stat-label">Enabled</p>
            </div>
            <div class="stat-card">
              <p class="stat-value">{hostsStats.blocked_domains}</p>
              <p class="stat-label">Blocked Domains</p>
            </div>
          </div>

          <div class="card">
            <h3 class="font-semibold mb-4">Add Entry</h3>
            <div class="flex gap-4">
              <input
                type="text"
                class="input w-32"
                placeholder="IP Address"
                bind:value={newHostIp}
              />
              <input
                type="text"
                class="input flex-1"
                placeholder="Hostname (e.g. facebook.com)"
                bind:value={newHostname}
              />
              <button class="btn btn-primary" onclick={handleAddHost}
                >Add</button
              >
            </div>
          </div>

          <div class="card">
            <h3 class="font-semibold mb-4">Import Blocklist</h3>
            <div class="flex flex-wrap gap-2">
              {#each blocklists as [name, url]}
                <button
                  class="btn btn-secondary btn-sm"
                  onclick={() => handleImportBlocklist(url)}
                >
                  + {name}
                </button>
              {/each}
            </div>
          </div>

          {#if loadingHosts}
            <div class="flex justify-center py-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            <div class="bg-surface-800 rounded-xl overflow-hidden">
              <table class="w-full text-sm text-left">
                <thead class="bg-surface-700 text-gray-400">
                  <tr>
                    <th class="p-4">IP</th>
                    <th class="p-4">Hostnames</th>
                    <th class="p-4">Action</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-white/5">
                  {#each hostEntries as entry}
                    <tr
                      class="hover:bg-white/5 {entry.is_enabled
                        ? ''
                        : 'opacity-50'}"
                    >
                      <td class="p-4 font-mono">{entry.ip}</td>
                      <td class="p-4">{entry.hostnames.join(" ")}</td>
                      <td class="p-4 flex gap-2">
                        <button
                          class="btn btn-sm"
                          class:btn-secondary={entry.is_enabled}
                          onclick={() => handleToggleHost(entry.line_number)}
                        >
                          {entry.is_enabled ? "Disable" : "Enable"}
                        </button>
                        <button
                          class="btn btn-danger btn-sm"
                          onclick={() => handleRemoveHost(entry.line_number)}
                        >
                          Delete
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </main>
</div>
