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
  import * as dnsService from "./lib/services/dns";
  import { invoke } from "@tauri-apps/api/core";
  import logoImage from "./assets/logo.png";

  // ============================================================================
  // State (Svelte 5 Runes)
  // ============================================================================

  /** @type {'dashboard' | 'cleaner' | 'tweaks' | 'services' | 'startup' | 'packages' | 'processes' | 'repositories' | 'resources' | 'hosts' | 'gaming' | 'about'} */
  let currentPage = $state("dashboard");
  let loading = $state(true);

  // System stats
  let systemInfo = $state(null);
  let cpuStats = $state(null);
  let memoryStats = $state(null);
  let diskStats = $state([]);
  let distroInfo = $state(null);

  // Distro-specific
  let pmName = $state("apt"); // Package manager name
  let distroFamily = $state("Debian/Ubuntu");
  let repositoriesAvailable = $state(true); // APT repos only on Debian

  // Cleaner
  let cleanupCategories = $state([]);
  let totalReclaimable = $state(0);
  let cleaningCategory = $state(null);
  let autocleanConfig = $state({
    enabled: false,
    interval: "weekly",
    categories: ["trash", "thumbnails", "browser_cache"],
    last_run: null,
  });
  let autocleanStatus = $state("Not configured");
  let savingAutoclean = $state(false);

  // Tweaks
  let tweakCategories = $state([]);
  let applyingTweak = $state(null);

  // Services
  let services = $state([]);
  let servicesFilter = $state("");
  let loadingServices = $state(false);
  let selectedServiceCategory = $state("ALL");

  // Packages
  let packages = $state([]);
  let packagesFilter = $state("");
  let packageStats = $state([0, 0, 0]);
  let loadingPackages = $state(false);
  let selectedPackageCategory = $state("ALL");

  // Processes
  let processes = $state([]);
  let processesFilter = $state("");
  let loadingProcesses = $state(false);
  let selectedProcessCategory = $state("ALL");

  // Startup
  let startupApps = $state([]);
  let loadingStartup = $state(false);

  // Repositories
  let repositories = $state([]);
  let mirrors = $state([]);
  let loadingRepos = $state(false);
  let testingMirrors = $state(false);
  let newPpa = $state("");
  let selectedRegion = $state("ALL");
  let regionInfo = $state({
    detected_country: "",
    detected_code: "",
    available_regions: [],
  });
  let aptFastStatus = $state({
    installed: false,
    aria2_installed: false,
    max_connections: 5,
  });
  let installingAptFast = $state(false);
  let deletingRepo = $state(null);

  // Resources (enhanced with GPU, Disk I/O)
  let resourceHistory = $state({
    snapshots: [],
    net_rx_speed: [],
    net_tx_speed: [],
    disk_read_speed: [],
    disk_write_speed: [],
    ram_history: [],
  });
  let gpuInfo = $state(null);
  let resourceInterval = $state(null);

  // Ad-Block Manager
  let blocklistSources = $state([]);
  let adblockStats = $state({
    total_blocked_domains: 0,
    active_blocklists: [],
    hosts_file_size: 0,
  });
  let selectedBlocklists = $state([]);
  let loadingAdblock = $state(false);
  let applyingBlocklists = $state(false);

  // DNS Manager
  let dnsProviders = $state([]);
  let dnsStatus = $state({
    current_dns: [],
    active_provider: null,
  });
  let selectedDnsProvider = $state(null);
  let customDnsPrimary = $state("");
  let customDnsSecondary = $state("");
  let applyingDns = $state(false);

  // Gaming Center
  let gamingStatus = $state(null);
  let gamingPackages = $state([]);
  let gamingTweaks = $state([]);
  let gamingTab = $state("essentials"); // essentials, drivers, tweaks, onetouch
  let loadingGaming = $state(false);
  let installingPackage = $state(null);
  let applyingGamingTweak = $state(null);
  let systemProfile = $state(null);
  let gamingChecklist = $state(null);
  let runningOneTouch = $state(false);
  let oneTouchLogs = $state([]);

  // Refresh interval
  let refreshInterval = $state(null);

  // ============================================================================
  // Navigation
  // ============================================================================

  /** @type {{id: 'dashboard' | 'cleaner' | 'tweaks' | 'services' | 'startup' | 'packages' | 'processes' | 'repositories' | 'resources' | 'hosts' | 'gaming' , icon: string, label: string}[]} */
  const navItems = [
    // Overview
    { id: "dashboard", icon: "⬢", label: "Dashboard" },
    { id: "resources", icon: "◎", label: "System Monitor" },
    // System Management
    { id: "cleaner", icon: "✧", label: "System Cleaner" },
    { id: "tweaks", icon: "◈", label: "Performance" },
    { id: "processes", icon: "☰", label: "Process Manager" },
    // Software & Services
    { id: "packages", icon: "▣", label: "Packages" },
    { id: "repositories", icon: "⬡", label: "Repositories" },
    { id: "services", icon: "⚙", label: "Services" },
    { id: "startup", icon: "►", label: "Startup Apps" },
    // Network & Security
    { id: "hosts", icon: "⛊", label: "Ad-Block & DNS" },
    // Gaming
    { id: "gaming", icon: "🎮", label: "Gaming Center" },
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

      // Load distro-specific info
      try {
        pmName = await invoke("get_pm_name");
        distroFamily = await invoke("get_distro_family");
        repositoriesAvailable = await invoke("is_repositories_available");
      } catch (e) {
        console.warn("Failed to load distro info:", e);
      }
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
      [cleanupCategories, totalReclaimable, autocleanConfig, autocleanStatus] =
        await Promise.all([
          cleanerService.getCleanupCategories(),
          cleanerService.getTotalReclaimable(),
          cleanerService.getAutocleanSchedule(),
          cleanerService.getAutocleanStatus(),
        ]);
    } catch (e) {
      console.error("Failed to load cleaner:", e);
    }
  }

  async function handleSaveAutoclean() {
    savingAutoclean = true;
    try {
      await cleanerService.setAutocleanSchedule(autocleanConfig);
      autocleanStatus = await cleanerService.getAutocleanStatus();
    } catch (e) {
      console.error("Failed to save auto-clean:", e);
    }
    savingAutoclean = false;
  }

  function toggleAutocleanCategory(catId) {
    if (autocleanConfig.categories.includes(catId)) {
      autocleanConfig.categories = autocleanConfig.categories.filter(
        (c) => c !== catId,
      );
    } else {
      autocleanConfig.categories = [...autocleanConfig.categories, catId];
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
      [repositories, mirrors, regionInfo, aptFastStatus] = await Promise.all([
        reposService.getRepositories(),
        reposService.getMirrors(selectedRegion),
        reposService.getRegionInfo(),
        reposService.checkAptFast(),
      ]);
      // Auto-select detected region on first load
      if (selectedRegion === "ALL" && regionInfo.detected_code) {
        selectedRegion = regionInfo.detected_code;
        mirrors = await reposService.getMirrors(selectedRegion);
      }
    } catch (e) {
      console.error("Failed to load repositories:", e);
    }
    loadingRepos = false;
  }

  async function handleInstallAptFast() {
    installingAptFast = true;
    try {
      await reposService.installAptFast();
      aptFastStatus = await reposService.checkAptFast();
    } catch (e) {
      console.error("Failed to install apt-fast:", e);
    }
    installingAptFast = false;
  }

  async function handleDeleteRepo(filePath, isWholeFile) {
    deletingRepo = filePath;
    try {
      await reposService.deleteRepository(filePath, isWholeFile);
      await loadRepositories();
    } catch (e) {
      console.error("Failed to delete repo:", e);
    }
    deletingRepo = null;
  }

  async function handleRegionChange(region) {
    selectedRegion = region;
    mirrors = await reposService.getMirrors(region);
  }

  async function loadResources() {
    try {
      const snapshot = await resourcesService.getResourceSnapshot();
      await resourcesService.addResourceSnapshot(snapshot);
      resourceHistory = await resourcesService.getResourceHistory();

      // Load GPU info only once (or refresh occasionally)
      if (!gpuInfo) {
        gpuInfo = await resourcesService.getGpuInfo();
      }
    } catch (e) {
      console.error("Failed to load resources:", e);
    }
  }

  async function loadAdblock() {
    loadingAdblock = true;
    try {
      [blocklistSources, adblockStats] = await Promise.all([
        hostsService.getBlocklistSources(),
        hostsService.getAdBlockStats(),
      ]);
      // Pre-select enabled blocklists
      selectedBlocklists = blocklistSources
        .filter((s) => s.is_enabled)
        .map((s) => s.id);
    } catch (e) {
      console.error("Failed to load adblock:", e);
    }
    loadingAdblock = false;
  }

  async function loadDns() {
    try {
      [dnsProviders, dnsStatus] = await Promise.all([
        dnsService.getDnsProviders(),
        dnsService.getCurrentDns(),
      ]);
      selectedDnsProvider = dnsStatus.active_provider;
    } catch (e) {
      console.error("Failed to load dns:", e);
    }
  }

  async function loadHosts() {
    await Promise.all([loadAdblock(), loadDns()]);
  }

  async function loadGaming() {
    loadingGaming = true;
    try {
      [
        gamingStatus,
        gamingPackages,
        gamingTweaks,
        systemProfile,
        gamingChecklist,
      ] = await Promise.all([
        invoke("get_gaming_status"),
        invoke("get_gaming_packages"),
        invoke("get_gaming_tweaks"),
        invoke("get_system_profile"),
        invoke("get_gaming_checklist"),
      ]);
    } catch (e) {
      console.error("Failed to load gaming:", e);
    }
    loadingGaming = false;
  }

  async function handleOneTouchSetup() {
    runningOneTouch = true;
    oneTouchLogs = [];
    try {
      const logs = await invoke("one_touch_gaming_setup");
      oneTouchLogs = logs;
      await loadGaming(); // Refresh status
    } catch (e) {
      console.error("One-touch setup failed:", e);
      oneTouchLogs = ["❌ Error: " + e];
    }
    runningOneTouch = false;
  }

  async function handleInstallGamingPackage(pkgId) {
    installingPackage = pkgId;
    try {
      await invoke("install_gaming_package", { pkgId });
      await loadGaming();
    } catch (e) {
      console.error("Failed to install package:", e);
    }
    installingPackage = null;
  }

  async function handleApplyGamingTweak(tweakId, value) {
    applyingGamingTweak = tweakId;
    try {
      await invoke("apply_gaming_tweak", { tweakId, value });
      await loadGaming();
    } catch (e) {
      console.error("Failed to apply tweak:", e);
    }
    applyingGamingTweak = null;
  }

  async function handleApplyAllGamingTweaks() {
    applyingGamingTweak = "all";
    try {
      await invoke("apply_all_gaming_tweaks");
      await loadGaming();
    } catch (e) {
      console.error("Failed to apply all tweaks:", e);
    }
    applyingGamingTweak = null;
  }

  async function handleEnableMultilib() {
    try {
      await invoke("enable_multilib");
      await loadGaming();
    } catch (e) {
      console.error("Failed to enable multilib:", e);
    }
  }

  async function handleInstallVulkan() {
    try {
      await invoke("install_vulkan_support");
      await loadGaming();
    } catch (e) {
      console.error("Failed to install Vulkan:", e);
    }
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

  // Ad-Block Actions
  function toggleBlocklistSelection(id) {
    if (selectedBlocklists.includes(id)) {
      selectedBlocklists = selectedBlocklists.filter((s) => s !== id);
    } else {
      selectedBlocklists = [...selectedBlocklists, id];
    }
  }

  async function handleApplyBlocklists() {
    if (selectedBlocklists.length === 0) return;
    applyingBlocklists = true;
    try {
      const count = await hostsService.applyBlocklists(selectedBlocklists);
      console.log(`Applied ${count} blocked domains`);
      await loadAdblock();
    } catch (e) {
      console.error("Failed to apply blocklists:", e);
    }
    applyingBlocklists = false;
  }

  async function handleClearBlocklists() {
    applyingBlocklists = true;
    try {
      await hostsService.clearBlocklists();
      selectedBlocklists = [];
      await loadAdblock();
    } catch (e) {
      console.error("Failed to clear blocklists:", e);
    }
    applyingBlocklists = false;
  }

  // DNS Actions
  async function handleApplyDns() {
    applyingDns = true;
    try {
      if (selectedDnsProvider === "custom") {
        await dnsService.setCustomDns(customDnsPrimary, customDnsSecondary);
      } else if (selectedDnsProvider) {
        await dnsService.setDnsProvider(selectedDnsProvider);
      }
      await loadDns();
    } catch (e) {
      console.error("Failed to apply DNS:", e);
    }
    applyingDns = false;
  }

  async function handleResetDns() {
    applyingDns = true;
    try {
      await dnsService.resetDns();
      selectedDnsProvider = null;
      await loadDns();
    } catch (e) {
      console.error("Failed to reset DNS:", e);
    }
    applyingDns = false;
  }

  // ============================================================================
  // Lifecycle
  // ============================================================================

  onMount(() => {
    (async () => {
      await loadDashboard();
      loading = false;

      // Start refresh interval for stats
      refreshInterval = setInterval(refreshStats, 2000);
    })();

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
    else if (currentPage === "gaming") loadGaming();
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
  let filteredServices = $derived(
    services
      .filter(
        (s) =>
          selectedServiceCategory === "ALL" ||
          s.category === selectedServiceCategory,
      )
      .slice(0, 100),
  );
  let filteredPackages = $derived(
    packages
      .filter(
        (p) =>
          selectedPackageCategory === "ALL" ||
          p.category === selectedPackageCategory,
      )
      .slice(0, 100),
  );
  let filteredProcesses = $derived(
    processes
      .filter(
        (p) =>
          selectedProcessCategory === "ALL" ||
          p.category === selectedProcessCategory,
      )
      .slice(0, 100),
  );

  // Get unique categories for dropdowns
  let serviceCategories = $derived(
    [...new Set(services.map((s) => s.category))].sort(),
  );
  let packageCategories = $derived(
    [...new Set(packages.map((p) => p.category))].sort(),
  );
  let processCategories = $derived(
    [...new Set(processes.map((p) => p.category))].sort(),
  );
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
      <img src={logoImage} alt="Glance Logo" class="w-10 h-10 rounded-xl" />
      <div>
        <h1 class="font-bold text-lg text-gradient">Glance</h1>
        <p class="text-xs text-gray-500">v26.1.1</p>
      </div>
    </div>

    <nav class="flex-1 space-y-1">
      {#each navItems as item}
        <button
          class="nav-item w-full"
          class:active={currentPage === item.id}
          aria-current={currentPage === item.id ? "page" : undefined}
          onclick={() =>
            (currentPage = /** @type {typeof currentPage} */ (item.id))}
        >
          <span class="text-xl">{item.icon}</span>
          <span>{item.label}</span>
        </button>
      {/each}
    </nav>

    <!-- Sidebar Footer (clickable to About) -->
    <button
      class="mt-auto p-4 text-xs text-center w-full hover:bg-white/5 transition-colors cursor-pointer"
      onclick={() => (currentPage = "about")}
    >
      <p class="text-gray-500">Thank you, {systemInfo?.hostname ?? "User"}!</p>
      <p class="text-gray-600">Glance © 2025</p>
    </button>
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

          <!-- Auto-Clean Settings -->
          <div class="card mt-6">
            <h3 class="font-semibold mb-4 flex items-center gap-2">
              <span class="text-xl">⏰</span> Scheduled Cleaning
            </h3>

            <div class="space-y-4">
              <!-- Enable Toggle -->
              <div class="flex items-center justify-between">
                <div>
                  <p class="font-medium">Enable Auto-Clean</p>
                  <p class="text-sm text-gray-500">
                    Automatically clean selected categories on schedule
                  </p>
                </div>
                <button
                  class="toggle"
                  class:bg-primary-600={autocleanConfig.enabled}
                  onclick={() =>
                    (autocleanConfig.enabled = !autocleanConfig.enabled)}
                  aria-label="Toggle auto-clean"
                >
                  <span
                    class="toggle-dot"
                    class:translate-x-5={autocleanConfig.enabled}
                  ></span>
                </button>
              </div>

              {#if autocleanConfig.enabled}
                <!-- Interval Selection -->
                <div>
                  <p class="text-sm text-gray-400 mb-2">Clean every:</p>
                  <div class="flex gap-2">
                    {#each ["daily", "weekly", "monthly"] as interval}
                      <button
                        class="px-4 py-2 rounded-lg text-sm transition-all {autocleanConfig.interval ===
                        interval
                          ? 'bg-primary-600 text-white'
                          : 'bg-surface-700 text-gray-300 hover:bg-surface-600'}"
                        onclick={() => (autocleanConfig.interval = interval)}
                      >
                        {interval.charAt(0).toUpperCase() + interval.slice(1)}
                      </button>
                    {/each}
                  </div>
                </div>

                <!-- Category Selection -->
                <div>
                  <p class="text-sm text-gray-400 mb-2">
                    Categories to auto-clean:
                  </p>
                  <div class="grid grid-cols-2 gap-2">
                    {#each cleanupCategories.filter((c) => !c.requires_root) as category}
                      <label
                        class="flex items-center gap-2 p-2 rounded-lg bg-surface-800 hover:bg-surface-700 cursor-pointer"
                      >
                        <input
                          type="checkbox"
                          checked={autocleanConfig.categories.includes(
                            category.id,
                          )}
                          onchange={() => toggleAutocleanCategory(category.id)}
                          class="w-4 h-4 accent-primary-500"
                        />
                        <span class="text-sm"
                          >{category.icon} {category.name}</span
                        >
                      </label>
                    {/each}
                  </div>
                </div>

                <!-- Status -->
                <div class="text-sm text-gray-400">
                  Status: <span class="text-white">{autocleanStatus}</span>
                </div>
              {/if}

              <!-- Save Button -->
              <button
                class="btn btn-primary w-full"
                onclick={handleSaveAutoclean}
                disabled={savingAutoclean}
              >
                {#if savingAutoclean}
                  <span class="spinner"></span>
                {:else}
                  {autocleanConfig.enabled ? "Save & Enable" : "Save & Disable"}
                {/if}
              </button>
            </div>
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
              <div class="space-y-4">
                {#each category.tweaks as tweak}
                  <div class="p-3 bg-surface-800/50 rounded-lg">
                    <div class="flex items-center justify-between mb-2">
                      <div class="flex items-center gap-2">
                        <span class="font-medium">{tweak.name}</span>
                        {#if tweak.is_applied}
                          <span class="badge badge-success text-xs"
                            >Applied</span
                          >
                        {/if}
                      </div>
                      {#if tweak.tweak_type === "slider"}
                        <span class="text-sm text-gray-400">
                          Current: <span class="text-white"
                            >{tweak.current_value}</span
                          >
                          <span class="mx-1">•</span>
                          Rec:
                          <span class="text-accent-400"
                            >{tweak.recommended_value}</span
                          >
                        </span>
                      {/if}
                    </div>
                    <p class="text-xs text-gray-500 mb-3">
                      {tweak.description}
                    </p>

                    {#if tweak.tweak_type === "slider" && tweak.min_value !== null && tweak.max_value !== null}
                      <!-- Slider UI -->
                      <div class="flex items-center gap-3">
                        <span class="text-xs text-gray-500 w-8"
                          >{tweak.min_value}</span
                        >
                        <input
                          type="range"
                          min={tweak.min_value}
                          max={tweak.max_value}
                          value={tweak.current_value}
                          class="flex-1 h-2 bg-surface-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                          onchange={(e) =>
                            handleApplyTweak(
                              tweak.id,
                              Number(
                                /** @type {HTMLInputElement} */ (e.target)
                                  .value,
                              ),
                            )}
                        />
                        <span class="text-xs text-gray-500 w-8 text-right"
                          >{tweak.max_value}</span
                        >
                        <button
                          class="btn btn-sm btn-primary"
                          disabled={applyingTweak === tweak.id}
                          onclick={() =>
                            handleApplyTweak(tweak.id, tweak.recommended_value)}
                        >
                          {#if applyingTweak === tweak.id}
                            <span class="spinner"></span>
                          {:else}
                            Set Recommended
                          {/if}
                        </button>
                      </div>
                    {:else if tweak.tweak_type === "selector" && tweak.options}
                      <!-- Selector/Dropdown UI -->
                      <div class="flex items-center gap-2 flex-wrap">
                        {#each tweak.options as option}
                          <button
                            class="px-3 py-1.5 rounded-lg text-sm transition-all {tweak.current_value ===
                            option
                              ? 'bg-primary-600 text-white'
                              : 'bg-surface-700 text-gray-300 hover:bg-surface-600'}"
                            disabled={applyingTweak === tweak.id}
                            onclick={() => handleApplyTweak(tweak.id, option)}
                          >
                            {#if tweak.id === "rmem_max" || tweak.id === "wmem_max"}
                              {parseInt(option) >= 1048576
                                ? `${(parseInt(option) / 1048576).toFixed(0)} MB`
                                : option}
                            {:else}
                              {option}
                            {/if}
                            {#if option === tweak.recommended_value}
                              <span class="ml-1 text-xs text-accent-300">★</span
                              >
                            {/if}
                          </button>
                        {/each}
                      </div>
                    {:else if tweak.tweak_type === "preset" && tweak.options}
                      <!-- CPU Governor Preset Buttons -->
                      <div class="flex gap-2">
                        {#each ["powersave", "schedutil", "performance"] as mode}
                          {#if tweak.options.includes(mode) || (mode === "schedutil" && tweak.options.includes("ondemand"))}
                            {@const actualMode =
                              mode === "schedutil" &&
                              !tweak.options.includes("schedutil")
                                ? "ondemand"
                                : mode}
                            <button
                              class="flex-1 py-3 rounded-lg text-center transition-all {tweak.current_value ===
                              actualMode
                                ? 'bg-primary-600 text-white ring-2 ring-primary-400'
                                : 'bg-surface-700 text-gray-300 hover:bg-surface-600'}"
                              disabled={applyingTweak === tweak.id}
                              onclick={() =>
                                handleApplyTweak(tweak.id, actualMode)}
                            >
                              <div class="text-2xl mb-1">
                                {mode === "powersave"
                                  ? "🔋"
                                  : mode === "schedutil"
                                    ? "⚖️"
                                    : "⚡"}
                              </div>
                              <div class="text-sm font-medium">
                                {mode === "powersave"
                                  ? "Power Saver"
                                  : mode === "schedutil"
                                    ? "Balanced"
                                    : "Performance"}
                              </div>
                              {#if mode === "performance"}
                                <div class="text-xs text-accent-300 mt-1">
                                  Recommended
                                </div>
                              {/if}
                            </button>
                          {/if}
                        {/each}
                      </div>
                    {:else if tweak.tweak_type === "toggle"}
                      <!-- Toggle Switch (for ZRAM etc) -->
                      <div class="flex gap-2">
                        <button
                          class="flex-1 py-3 rounded-lg text-center transition-all {tweak.current_value ===
                          'enabled'
                            ? 'bg-success-600 text-white ring-2 ring-success-400'
                            : 'bg-surface-700 text-gray-300 hover:bg-surface-600'}"
                          disabled={applyingTweak === tweak.id}
                          onclick={() => handleApplyTweak(tweak.id, "enabled")}
                        >
                          <div class="text-xl">✓</div>
                          <div class="text-sm font-medium">Enabled</div>
                        </button>
                        <button
                          class="flex-1 py-3 rounded-lg text-center transition-all {tweak.current_value ===
                          'disabled'
                            ? 'bg-danger-600 text-white ring-2 ring-danger-400'
                            : 'bg-surface-700 text-gray-300 hover:bg-surface-600'}"
                          disabled={applyingTweak === tweak.id}
                          onclick={() => handleApplyTweak(tweak.id, "disabled")}
                        >
                          <div class="text-xl">✕</div>
                          <div class="text-sm font-medium">Disabled</div>
                        </button>
                      </div>
                    {:else}
                      <!-- Fallback: Simple Apply Button -->
                      <button
                        class="btn btn-primary btn-sm"
                        disabled={applyingTweak === tweak.id ||
                          tweak.is_applied}
                        onclick={() =>
                          handleApplyTweak(tweak.id, tweak.recommended_value)}
                      >
                        {#if applyingTweak === tweak.id}
                          <span class="spinner"></span>
                        {:else if tweak.is_applied}
                          ✓ Applied
                        {:else}
                          Apply Recommended
                        {/if}
                      </button>
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
            <select
              class="bg-surface-800 text-white border border-white/20 rounded-lg px-4 py-2 text-sm cursor-pointer"
              bind:value={selectedServiceCategory}
            >
              <option value="ALL">📋 All Categories</option>
              {#each serviceCategories as cat}
                <option value={cat}>{cat}</option>
              {/each}
            </select>
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
                      <span
                        class="badge"
                        style="background: rgba(99,102,241,0.2); color: #818cf8;"
                        >{service.category}</span
                      >
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
                    aria-label={`Toggle ${app.name} startup`}
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
            <select
              class="bg-surface-800 text-white border border-white/20 rounded-lg px-4 py-2 text-sm cursor-pointer"
              bind:value={selectedPackageCategory}
            >
              <option value="ALL">📦 All Categories</option>
              {#each packageCategories as cat}
                <option value={cat}>{cat}</option>
              {/each}
            </select>
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
                      <span
                        class="badge"
                        style="background: rgba(236,72,153,0.2); color: #f472b6;"
                        >{pkg.category}</span
                      >
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
            <select
              class="bg-surface-800 text-white border border-white/20 rounded-lg px-4 py-2 text-sm cursor-pointer"
              bind:value={selectedProcessCategory}
            >
              <option value="ALL">🔄 All Processes</option>
              {#each processCategories as cat}
                <option value={cat}>{cat}</option>
              {/each}
            </select>
            <button class="btn btn-secondary" onclick={loadProcesses}>
              Refresh
            </button>
            <button
              class="btn btn-danger"
              onclick={async () => {
                const result = await invoke("bulk_terminate_apps");
                alert(result.message);
                loadProcesses();
              }}
              title="Terminate all app processes to free RAM"
            >
              🧹 Clean Apps RAM
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
                      <span
                        class="badge"
                        style="background: rgba(34,197,94,0.2); color: #4ade80;"
                        >{proc.category}</span
                      >
                      <span class="badge badge-info">PID: {proc.pid}</span>
                    </div>
                    <div class="flex gap-4 text-sm text-gray-500 mt-1">
                      <span>CPU: {proc.cpu_usage.toFixed(1)}%</span>
                      <span>RAM: {formatBytes(proc.memory_bytes)}</span>
                      <span class="truncate max-w-xs">{proc.command}</span>
                    </div>
                  </div>
                  {#if proc.is_killable}
                    <div class="flex gap-2">
                      <button
                        class="btn btn-warning btn-sm"
                        onclick={() => handleKillProcess(proc.pid)}
                        title="Terminate (SIGTERM)"
                      >
                        Terminate
                      </button>
                      <button
                        class="btn btn-danger btn-sm"
                        onclick={async () => {
                          await invoke("force_kill_process", { pid: proc.pid });
                          loadProcesses();
                        }}
                        title="Force Kill (SIGKILL)"
                      >
                        Kill
                      </button>
                    </div>
                  {:else}
                    <span class="badge badge-danger text-xs">System</span>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if currentPage === "repositories"}
        <!-- Repositories -->
        <div class="space-y-6">
          {#if !repositoriesAvailable}
            <!-- Not available on this distro -->
            <div class="card text-center py-12">
              <span class="text-5xl mb-4 block">🚫</span>
              <h2 class="text-xl font-semibold mb-2">Not Available</h2>
              <p class="text-gray-400">
                APT Repository Manager is only available on Debian/Ubuntu based
                systems.
              </p>
              <p class="text-gray-500 text-sm mt-2">Detected: {distroFamily}</p>
            </div>
          {:else}
            <!-- apt-fast Card -->
            <div
              class="card bg-gradient-to-r from-primary-900/50 to-accent-900/50 border border-primary-500/30"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <span class="text-3xl">🚀</span>
                  <div>
                    <h3 class="font-semibold">apt-fast</h3>
                    <p class="text-sm text-gray-400">
                      Parallel downloads for faster package installation
                    </p>
                  </div>
                </div>
                {#if aptFastStatus.installed}
                  <div class="flex items-center gap-3">
                    <span class="badge badge-success">Installed</span>
                    <span class="text-sm text-gray-400"
                      >{aptFastStatus.max_connections} connections</span
                    >
                  </div>
                {:else}
                  <button
                    class="btn btn-primary"
                    onclick={handleInstallAptFast}
                    disabled={installingAptFast}
                  >
                    {#if installingAptFast}
                      <span class="spinner"></span> Installing...
                    {:else}
                      Install apt-fast
                    {/if}
                  </button>
                {/if}
              </div>
            </div>

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
                  <div class="flex items-center gap-4">
                    <h4 class="font-medium">{distroFamily} Mirrors</h4>
                    <!-- Region Dropdown -->
                    <select
                      class="bg-surface-800 text-white border border-white/20 rounded-lg px-4 py-2 text-sm cursor-pointer hover:border-primary-500 focus:border-primary-500 focus:outline-none"
                      bind:value={selectedRegion}
                      onchange={(e) =>
                        handleRegionChange(e.currentTarget.value)}
                    >
                      <option value="ALL">🌍 All Regions</option>
                      {#each regionInfo.available_regions as [code, name]}
                        <option value={code}>{name} ({code})</option>
                      {/each}
                    </select>
                    {#if regionInfo.detected_code}
                      <span class="text-xs text-gray-500">
                        Detected: {regionInfo.detected_country}
                      </span>
                    {/if}
                  </div>
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

                <div class="grid grid-cols-2 gap-2 max-h-60 overflow-y-auto">
                  {#each mirrors as mirror}
                    <button
                      class="flex items-center justify-between p-3 bg-surface-800 hover:bg-surface-700 rounded-lg text-left transition-all"
                      onclick={() => handleSetMirror(mirror.uri)}
                    >
                      <div class="flex flex-col min-w-0">
                        <span class="text-sm font-medium truncate"
                          >{mirror.name}</span
                        >
                        <span class="text-xs text-gray-500 truncate"
                          >{mirror.uri}</span
                        >
                      </div>
                      <div class="flex items-center gap-2 ml-2 shrink-0">
                        <span class="text-xs text-gray-400"
                          >{mirror.country_code}</span
                        >
                        {#if mirror.latency_ms !== null}
                          <span
                            class="text-xs font-mono px-2 py-0.5 rounded {mirror.latency_ms <
                            100
                              ? 'bg-green-500/20 text-green-400'
                              : mirror.latency_ms < 300
                                ? 'bg-yellow-500/20 text-yellow-400'
                                : 'bg-red-500/20 text-red-400'}"
                          >
                            {mirror.latency_ms}ms
                          </span>
                        {/if}
                      </div>
                    </button>
                  {/each}
                </div>
                {#if mirrors.length === 0}
                  <p class="text-center text-gray-500 py-4">
                    No mirrors for selected region
                  </p>
                {/if}
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
                        aria-label="Toggle repository"
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
                          disabled={deletingRepo === repo.file_path}
                          onclick={() => handleDeleteRepo(repo.file_path, true)}
                          aria-label="Delete repository"
                        >
                          {#if deletingRepo === repo.file_path}
                            <span class="spinner w-3 h-3"></span>
                          {:else}
                            🗑️
                          {/if}
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          {/if}
        </div>
      {:else if currentPage === "resources"}
        <!-- Enhanced Resources Monitor -->
        <div class="space-y-6">
          <!-- CPU Section -->
          <div class="card">
            <div class="flex items-center justify-between mb-4">
              <h3 class="font-semibold flex items-center gap-2">
                <span class="text-xl">⚡</span> CPU
              </h3>
              <div class="text-right">
                <span class="text-2xl font-bold text-gradient">
                  {resourceHistory.snapshots.length > 0
                    ? resourceHistory.snapshots[
                        resourceHistory.snapshots.length - 1
                      ].cpu_percent.toFixed(0)
                    : 0}%
                </span>
              </div>
            </div>

            <!-- CPU Info -->
            {#if systemInfo}
              <div class="mb-4 text-sm text-gray-400">
                <span class="text-white font-medium"
                  >{systemInfo.cpu_model}</span
                >
                <span class="mx-2">•</span>
                <span
                  >{systemInfo.cpu_cores} Cores / {systemInfo.cpu_threads} Threads</span
                >
                {#if cpuStats?.frequency_mhz}
                  <span class="mx-2">•</span>
                  <span>{(cpuStats.frequency_mhz / 1000).toFixed(2)} GHz</span>
                {/if}
              </div>
            {/if}

            <!-- Per-Core Usage Bars -->
            {#if resourceHistory.snapshots.length > 0}
              {@const latestSnapshot =
                resourceHistory.snapshots[resourceHistory.snapshots.length - 1]}
              {#if latestSnapshot.per_core_percent && latestSnapshot.per_core_percent.length > 0}
                <div class="grid grid-cols-8 gap-2 mb-4">
                  {#each latestSnapshot.per_core_percent as coreUsage, i}
                    <div class="text-center">
                      <div
                        class="h-16 bg-surface-800 rounded-lg relative overflow-hidden"
                      >
                        <div
                          class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-primary-600 to-primary-400 transition-all duration-300"
                          style="height: {coreUsage}%"
                        ></div>
                      </div>
                      <span class="text-xs text-gray-500 mt-1 block"
                        >{coreUsage.toFixed(0)}%</span
                      >
                    </div>
                  {/each}
                </div>
              {/if}
            {/if}

            <!-- CPU Graph with Grid Lines -->
            <div
              class="relative h-32 bg-surface-900/50 rounded-lg overflow-hidden"
            >
              <!-- Grid lines -->
              <div
                class="absolute inset-0 flex flex-col justify-between pointer-events-none"
              >
                <div
                  class="border-b border-surface-700/50 h-0 flex items-center"
                >
                  <span class="text-[10px] text-gray-600 ml-1">100%</span>
                </div>
                <div
                  class="border-b border-surface-700/50 h-0 flex items-center"
                >
                  <span class="text-[10px] text-gray-600 ml-1">75%</span>
                </div>
                <div
                  class="border-b border-surface-700/50 h-0 flex items-center"
                >
                  <span class="text-[10px] text-gray-600 ml-1">50%</span>
                </div>
                <div
                  class="border-b border-surface-700/50 h-0 flex items-center"
                >
                  <span class="text-[10px] text-gray-600 ml-1">25%</span>
                </div>
                <div class="h-0"></div>
              </div>
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
              <div
                class="absolute inset-0 bg-gradient-to-t from-primary-500/20 to-transparent pointer-events-none"
              ></div>
            </div>
            <div class="flex justify-between mt-2 text-xs text-gray-500">
              <span>60s ago</span>
              <span>Now</span>
            </div>
          </div>

          <!-- GPU Section -->
          {#if gpuInfo}
            <div class="card">
              <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold flex items-center gap-2">
                  <span class="text-xl">�</span> GPU
                </h3>
                {#if gpuInfo.usage_percent !== null}
                  <span class="text-lg font-bold text-gradient"
                    >{gpuInfo.usage_percent.toFixed(0)}%</span
                  >
                {/if}
              </div>

              <div class="text-sm text-gray-400 mb-4">
                <span class="text-white font-medium">{gpuInfo.name}</span>
                {#if gpuInfo.driver_version}
                  <span class="mx-2">•</span>
                  <span>Driver: {gpuInfo.driver_version}</span>
                {/if}
              </div>

              {#if gpuInfo.vram_total_mb > 0}
                <div class="mb-4">
                  <div class="flex justify-between text-sm mb-1">
                    <span class="text-gray-400">VRAM</span>
                    <span
                      >{gpuInfo.vram_used_mb} MB / {gpuInfo.vram_total_mb} MB</span
                    >
                  </div>
                  <div class="h-4 bg-surface-800 rounded-full overflow-hidden">
                    <div
                      class="h-full bg-gradient-to-r from-emerald-600 to-emerald-400"
                      style="width: {(gpuInfo.vram_used_mb /
                        gpuInfo.vram_total_mb) *
                        100}%"
                    ></div>
                  </div>
                </div>
              {/if}

              {#if gpuInfo.temperature_c !== null}
                <div class="flex items-center gap-4 text-sm">
                  <span class="text-gray-400">Temperature:</span>
                  <span
                    class="font-medium {gpuInfo.temperature_c > 80
                      ? 'text-red-400'
                      : gpuInfo.temperature_c > 60
                        ? 'text-yellow-400'
                        : 'text-emerald-400'}"
                  >
                    {gpuInfo.temperature_c.toFixed(0)}°C
                  </span>
                </div>
              {/if}
            </div>
          {/if}

          <!-- Network Section -->
          <div class="card">
            <div class="flex items-center justify-between mb-4">
              <h3 class="font-semibold flex items-center gap-2">
                <span class="text-xl">🌐</span> Network
              </h3>
              <div class="flex gap-4 text-sm">
                <span class="text-emerald-400">
                  ⬇ {formatBytes(
                    resourceHistory.net_rx_speed[
                      resourceHistory.net_rx_speed.length - 1
                    ] || 0,
                  )}/s
                </span>
                <span class="text-blue-400">
                  ⬆ {formatBytes(
                    resourceHistory.net_tx_speed[
                      resourceHistory.net_tx_speed.length - 1
                    ] || 0,
                  )}/s
                </span>
              </div>
            </div>

            <div
              class="relative h-32 bg-surface-900/50 rounded-lg overflow-hidden"
            >
              <!-- Grid lines -->
              <div
                class="absolute inset-0 flex flex-col justify-between pointer-events-none"
              >
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="h-0"></div>
              </div>
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
                        ...resourceHistory.net_tx_speed,
                        1024,
                      );
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
                        ...resourceHistory.net_rx_speed,
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
            </div>

            <!-- Legend -->
            <div class="flex justify-center gap-6 mt-2 text-xs text-gray-500">
              <span class="flex items-center gap-1"
                ><span class="w-3 h-1 bg-emerald-500 rounded"></span> Download</span
              >
              <span class="flex items-center gap-1"
                ><span class="w-3 h-1 bg-blue-500 rounded"></span> Upload</span
              >
            </div>
          </div>

          <!-- Disk I/O Section -->
          <div class="card">
            <div class="flex items-center justify-between mb-4">
              <h3 class="font-semibold flex items-center gap-2">
                <span class="text-xl">💾</span> Disk I/O
              </h3>
              <div class="flex gap-4 text-sm">
                <span class="text-cyan-400">
                  📖 {formatBytes(
                    resourceHistory.disk_read_speed[
                      resourceHistory.disk_read_speed.length - 1
                    ] || 0,
                  )}/s
                </span>
                <span class="text-orange-400">
                  ✏️ {formatBytes(
                    resourceHistory.disk_write_speed[
                      resourceHistory.disk_write_speed.length - 1
                    ] || 0,
                  )}/s
                </span>
              </div>
            </div>

            <div
              class="relative h-32 bg-surface-900/50 rounded-lg overflow-hidden"
            >
              <!-- Grid lines -->
              <div
                class="absolute inset-0 flex flex-col justify-between pointer-events-none"
              >
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="border-b border-surface-700/30 h-0"></div>
                <div class="h-0"></div>
              </div>
              <svg
                class="w-full h-full"
                viewBox="0 0 100 100"
                preserveAspectRatio="none"
              >
                <!-- Read (Cyan) -->
                <polyline
                  vector-effect="non-scaling-stroke"
                  points={resourceHistory.disk_read_speed
                    .map((s, i) => {
                      const max = Math.max(
                        ...resourceHistory.disk_read_speed,
                        ...resourceHistory.disk_write_speed,
                        1024,
                      );
                      return `${(i / Math.max(1, resourceHistory.disk_read_speed.length - 1)) * 100},${100 - (s / max) * 90}`;
                    })
                    .join(" ")}
                  fill="none"
                  stroke="#06b6d4"
                  stroke-width="2"
                />
                <!-- Write (Orange) -->
                <polyline
                  vector-effect="non-scaling-stroke"
                  points={resourceHistory.disk_write_speed
                    .map((s, i) => {
                      const max = Math.max(
                        ...resourceHistory.disk_read_speed,
                        ...resourceHistory.disk_write_speed,
                        1024,
                      );
                      return `${(i / Math.max(1, resourceHistory.disk_write_speed.length - 1)) * 100},${100 - (s / max) * 90}`;
                    })
                    .join(" ")}
                  fill="none"
                  stroke="#f97316"
                  stroke-width="2"
                />
              </svg>
            </div>

            <!-- Legend -->
            <div class="flex justify-center gap-6 mt-2 text-xs text-gray-500">
              <span class="flex items-center gap-1"
                ><span class="w-3 h-1 bg-cyan-500 rounded"></span> Read</span
              >
              <span class="flex items-center gap-1"
                ><span class="w-3 h-1 bg-orange-500 rounded"></span> Write</span
              >
            </div>
          </div>
        </div>
      {:else if currentPage === "hosts"}
        <!-- Ad-Block & DNS Manager -->
        <div class="space-y-6">
          <!-- Stats -->
          <div class="grid grid-cols-3 gap-4">
            <div class="stat-card">
              <p class="stat-value">
                {adblockStats.total_blocked_domains.toLocaleString()}
              </p>
              <p class="stat-label">Blocked Domains</p>
            </div>
            <div class="stat-card">
              <p class="stat-value">{adblockStats.active_blocklists.length}</p>
              <p class="stat-label">Active Lists</p>
            </div>
            <div class="stat-card">
              <p class="stat-value">
                {formatBytes(adblockStats.hosts_file_size)}
              </p>
              <p class="stat-label">Hosts File Size</p>
            </div>
          </div>

          <!-- Ad-Block Section -->
          <div class="card">
            <div class="flex items-center justify-between mb-4">
              <h3 class="font-semibold flex items-center gap-2">
                <span class="text-xl">🛡️</span> Ad-Block Manager
              </h3>
              <div class="flex gap-2">
                <button
                  class="btn btn-danger btn-sm"
                  disabled={applyingBlocklists ||
                    adblockStats.total_blocked_domains === 0}
                  onclick={handleClearBlocklists}
                >
                  {#if applyingBlocklists}
                    <span class="spinner w-4 h-4"></span>
                  {:else}
                    Clear All
                  {/if}
                </button>
                <button
                  class="btn btn-primary"
                  disabled={applyingBlocklists ||
                    selectedBlocklists.length === 0}
                  onclick={handleApplyBlocklists}
                >
                  {#if applyingBlocklists}
                    <span class="spinner w-4 h-4 mr-2"></span> Applying...
                  {:else}
                    Apply Selected ({selectedBlocklists.length})
                  {/if}
                </button>
              </div>
            </div>

            <p class="text-sm text-gray-400 mb-4">
              Select blocklists from trusted sources. Changes will be applied to <code
                class="text-primary-400">/etc/hosts</code
              >.
            </p>

            {#if loadingAdblock}
              <div class="flex justify-center py-8">
                <div class="spinner w-8 h-8"></div>
              </div>
            {:else}
              <div class="space-y-2">
                {#each blocklistSources as source}
                  <div
                    class="list-item cursor-pointer {selectedBlocklists.includes(
                      source.id,
                    )
                      ? 'border-primary-500 bg-primary-500/10'
                      : ''}"
                    onclick={() => toggleBlocklistSelection(source.id)}
                    role="checkbox"
                    aria-checked={selectedBlocklists.includes(source.id)}
                    tabindex="0"
                    onkeydown={(e) =>
                      e.key === "Enter" && toggleBlocklistSelection(source.id)}
                  >
                    <div class="flex items-center gap-3">
                      <div
                        class="w-5 h-5 rounded border-2 flex items-center justify-center transition-colors"
                        class:border-primary-500={selectedBlocklists.includes(
                          source.id,
                        )}
                        class:bg-primary-500={selectedBlocklists.includes(
                          source.id,
                        )}
                        class:border-gray-500={!selectedBlocklists.includes(
                          source.id,
                        )}
                      >
                        {#if selectedBlocklists.includes(source.id)}
                          <span class="text-white text-xs">✓</span>
                        {/if}
                      </div>
                      <div class="flex-1">
                        <div class="flex items-center gap-2">
                          <span class="font-medium">{source.name}</span>
                          {#if source.is_enabled}
                            <span class="badge badge-success">Active</span>
                          {/if}
                        </div>
                        <p class="text-sm text-gray-500">
                          {source.description}
                        </p>
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <!-- DNS Section -->
          <div class="card">
            <div class="flex items-center justify-between mb-4">
              <h3 class="font-semibold flex items-center gap-2">
                <span class="text-xl">🌐</span> DNS Manager
              </h3>
              <div class="flex gap-2">
                <button
                  class="btn btn-secondary btn-sm"
                  disabled={applyingDns}
                  onclick={handleResetDns}
                >
                  Reset to DHCP
                </button>
                <button
                  class="btn btn-primary"
                  disabled={applyingDns || !selectedDnsProvider}
                  onclick={handleApplyDns}
                >
                  {#if applyingDns}
                    <span class="spinner w-4 h-4 mr-2"></span> Applying...
                  {:else}
                    Apply DNS
                  {/if}
                </button>
              </div>
            </div>

            <div class="mb-4 p-3 bg-surface-800 rounded-lg">
              <p class="text-sm text-gray-400">
                Current DNS:
                <span class="text-white font-mono">
                  {dnsStatus.current_dns.length > 0
                    ? dnsStatus.current_dns.join(", ")
                    : "Automatic (DHCP)"}
                </span>
                {#if dnsStatus.active_provider}
                  <span class="badge badge-info ml-2"
                    >{dnsProviders.find(
                      (p) => p.id === dnsStatus.active_provider,
                    )?.name || dnsStatus.active_provider}</span
                  >
                {/if}
              </p>
            </div>

            <!-- DNS Categories -->
            {#each ["general", "adblock", "security", "family"] as category}
              {@const categoryProviders = dnsProviders.filter(
                (p) => p.category === category,
              )}
              {#if categoryProviders.length > 0}
                <div class="mb-4">
                  <h4
                    class="text-sm font-medium text-gray-400 mb-2 uppercase tracking-wide"
                  >
                    {category === "general"
                      ? "⚡ General"
                      : category === "adblock"
                        ? "🚫 Ad-Blocking"
                        : category === "security"
                          ? "🔒 Security"
                          : "👨‍👩‍👧‍👦 Family Safe"}
                  </h4>
                  <div class="grid grid-cols-2 gap-2">
                    {#each categoryProviders as provider}
                      <div
                        class="p-3 rounded-lg border cursor-pointer transition-all {selectedDnsProvider ===
                        provider.id
                          ? 'border-primary-500 bg-primary-500/10'
                          : 'border-surface-600 hover:border-surface-500'}"
                        onclick={() => (selectedDnsProvider = provider.id)}
                        role="radio"
                        aria-checked={selectedDnsProvider === provider.id}
                        tabindex="0"
                        onkeydown={(e) =>
                          e.key === "Enter" &&
                          (selectedDnsProvider = provider.id)}
                      >
                        <div class="flex items-center gap-2">
                          <div
                            class="w-4 h-4 rounded-full border-2 flex items-center justify-center"
                            class:border-primary-500={selectedDnsProvider ===
                              provider.id}
                            class:border-gray-500={selectedDnsProvider !==
                              provider.id}
                          >
                            {#if selectedDnsProvider === provider.id}
                              <div
                                class="w-2 h-2 rounded-full bg-primary-500"
                              ></div>
                            {/if}
                          </div>
                          <div class="flex-1">
                            <span class="font-medium text-sm"
                              >{provider.name}</span
                            >
                            <p class="text-xs text-gray-500 font-mono">
                              {provider.primary_dns}
                            </p>
                          </div>
                        </div>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            {/each}

            <!-- Custom DNS -->
            <div class="mt-4 pt-4 border-t border-surface-700">
              <h4
                class="text-sm font-medium text-gray-400 mb-2 uppercase tracking-wide"
              >
                🔧 Custom DNS
              </h4>
              <div class="flex gap-4">
                <input
                  type="text"
                  class="input flex-1"
                  placeholder="Primary DNS (e.g. 1.1.1.1)"
                  bind:value={customDnsPrimary}
                  onfocus={() => (selectedDnsProvider = "custom")}
                />
                <input
                  type="text"
                  class="input flex-1"
                  placeholder="Secondary DNS (optional)"
                  bind:value={customDnsSecondary}
                  onfocus={() => (selectedDnsProvider = "custom")}
                />
              </div>
            </div>
          </div>
        </div>
      {:else if currentPage === "gaming"}
        <!-- Gaming Center -->
        <div class="space-y-6">
          <!-- Gaming Status Header -->
          {#if loadingGaming}
            <div class="flex items-center justify-center p-8">
              <div class="spinner w-8 h-8"></div>
            </div>
          {:else}
            {#if gamingStatus}
              <div class="card">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-4">
                    <div class="text-4xl">🎮</div>
                    <div>
                      <h3 class="text-lg font-semibold">
                        {gamingStatus.gpu?.model
                          ?.split("[")
                          .pop()
                          ?.replace("]", "") ||
                          gamingStatus.gpu?.model ||
                          "GPU Not Detected"}
                      </h3>
                      <p class="text-gray-400 text-sm">
                        {gamingStatus.gpu?.vendor?.toUpperCase() || "Unknown"} •
                        Driver: {gamingStatus.gpu?.driver || "N/A"}
                        {gamingStatus.gpu?.driver_version
                          ? `v${gamingStatus.gpu.driver_version}`
                          : ""} • Vulkan: {gamingStatus.gpu?.vulkan_ready
                          ? "✓ Ready"
                          : "✗ Not Ready"}
                      </p>
                    </div>
                  </div>
                  <div class="flex items-center gap-3">
                    <span class="text-sm text-gray-500">Gaming Score:</span>
                    <span
                      class="px-4 py-2 rounded-lg font-semibold {gamingStatus.score_color ===
                      'green'
                        ? 'bg-green-500/20 text-green-400'
                        : gamingStatus.score_color === 'yellow'
                          ? 'bg-yellow-500/20 text-yellow-400'
                          : 'bg-red-500/20 text-red-400'}"
                    >
                      {gamingStatus.gaming_score}
                    </span>
                  </div>
                </div>

                <!-- Issues -->
                {#if gamingStatus.issues?.length > 0}
                  <div class="mt-4 pt-4 border-t border-surface-700">
                    <p class="text-sm text-gray-400 mb-2">Issues to Fix:</p>
                    <div class="space-y-1">
                      {#each gamingStatus.issues as issue}
                        <p class="text-sm text-yellow-400">⚠ {issue}</p>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            {/if}

            <!-- Tab Navigation -->
            <div class="flex gap-2 bg-surface-800 p-1 rounded-lg w-fit">
              <button
                class="px-4 py-2 rounded-md text-sm font-medium transition-all {gamingTab ===
                'essentials'
                  ? 'bg-primary-500 text-white'
                  : 'text-gray-400 hover:text-white'}"
                onclick={() => (gamingTab = "essentials")}
              >
                📦 Essentials & Launchers
              </button>
              <button
                class="px-4 py-2 rounded-md text-sm font-medium transition-all {gamingTab ===
                'drivers'
                  ? 'bg-primary-500 text-white'
                  : 'text-gray-400 hover:text-white'}"
                onclick={() => (gamingTab = "drivers")}
              >
                💻 Drivers & System
              </button>
              <button
                class="px-4 py-2 rounded-md text-sm font-medium transition-all {gamingTab ===
                'tweaks'
                  ? 'bg-primary-500 text-white'
                  : 'text-gray-400 hover:text-white'}"
                onclick={() => (gamingTab = "tweaks")}
              >
                🚀 Performance Tweaks
              </button>
              <button
                class="px-4 py-2 rounded-md text-sm font-medium transition-all {gamingTab ===
                'onetouch'
                  ? 'bg-green-500 text-white'
                  : 'text-gray-400 hover:text-white'}"
                onclick={() => (gamingTab = "onetouch")}
              >
                ⚡ One-Touch Setup
              </button>
            </div>

            <!-- Tab 1: Essentials & Launchers -->
            {#if gamingTab === "essentials"}
              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {#each gamingPackages.filter((p) => p.category === "platform" || p.category === "compatibility" || p.category === "tools") as pkg}
                  <div class="card card-hover">
                    <div class="flex items-start gap-3">
                      <span class="text-2xl">{pkg.icon}</span>
                      <div class="flex-1 min-w-0">
                        <div class="flex items-center gap-2">
                          <h4 class="font-semibold truncate">{pkg.name}</h4>
                          {#if pkg.recommended}
                            <span
                              class="px-2 py-0.5 text-xs bg-primary-500/20 text-primary-400 rounded"
                              >Recommended</span
                            >
                          {/if}
                        </div>
                        <p class="text-sm text-gray-400 mt-1 line-clamp-2">
                          {pkg.description}
                        </p>
                        <div class="flex items-center justify-between mt-3">
                          <span
                            class="text-xs {pkg.installed
                              ? 'text-green-400'
                              : 'text-gray-500'}"
                          >
                            {pkg.installed ? "✓ Installed" : "Not Installed"}
                          </span>
                          {#if !pkg.installed}
                            <button
                              class="btn btn-sm btn-primary"
                              disabled={installingPackage === pkg.id}
                              onclick={() => handleInstallGamingPackage(pkg.id)}
                            >
                              {installingPackage === pkg.id
                                ? "Installing..."
                                : "Install"}
                            </button>
                          {:else}
                            <span class="text-xs text-gray-600"
                              >via {pkg.install_method}</span
                            >
                          {/if}
                        </div>
                      </div>
                    </div>
                  </div>
                {/each}
              </div>

              <!-- Streaming Tools -->
              <h3 class="text-lg font-semibold mt-6 mb-4">
                📺 Streaming & Recording
              </h3>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                {#each gamingPackages.filter((p) => p.category === "streaming") as pkg}
                  <div class="card card-hover">
                    <div class="flex items-start gap-3">
                      <span class="text-2xl">{pkg.icon}</span>
                      <div class="flex-1">
                        <h4 class="font-semibold">{pkg.name}</h4>
                        <p class="text-sm text-gray-400 mt-1">
                          {pkg.description}
                        </p>
                        <div class="flex items-center justify-between mt-3">
                          <span
                            class="text-xs {pkg.installed
                              ? 'text-green-400'
                              : 'text-gray-500'}"
                          >
                            {pkg.installed ? "✓ Installed" : "Not Installed"}
                          </span>
                          {#if !pkg.installed}
                            <button
                              class="btn btn-sm btn-primary"
                              disabled={installingPackage === pkg.id}
                              onclick={() => handleInstallGamingPackage(pkg.id)}
                            >
                              {installingPackage === pkg.id
                                ? "Installing..."
                                : "Install"}
                            </button>
                          {/if}
                        </div>
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}

            <!-- Tab 2: Drivers & System -->
            {#if gamingTab === "drivers"}
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <!-- GPU Driver Status -->
                <div class="card">
                  <h3 class="section-title flex items-center gap-2">
                    {#if gamingStatus?.gpu?.vendor === "nvidia"}🟢
                    {:else if gamingStatus?.gpu?.vendor === "amd"}🔴
                    {:else}🔵{/if}
                    GPU Driver
                  </h3>
                  <div class="space-y-3">
                    <div
                      class="flex justify-between p-3 bg-surface-800 rounded-lg"
                    >
                      <span class="text-gray-400">Vendor</span>
                      <span class="font-medium"
                        >{gamingStatus?.gpu?.vendor?.toUpperCase() ||
                          "Unknown"}</span
                      >
                    </div>
                    <div
                      class="flex justify-between p-3 bg-surface-800 rounded-lg"
                    >
                      <span class="text-gray-400">Current Driver</span>
                      <span class="font-medium"
                        >{gamingStatus?.gpu?.driver || "Not detected"}</span
                      >
                    </div>
                    <div
                      class="flex justify-between p-3 bg-surface-800 rounded-lg"
                    >
                      <span class="text-gray-400">Driver Version</span>
                      <span class="font-medium"
                        >{gamingStatus?.gpu?.driver_version || "N/A"}</span
                      >
                    </div>
                    <div
                      class="flex justify-between p-3 bg-surface-800 rounded-lg"
                    >
                      <span class="text-gray-400">Vulkan Support</span>
                      <span
                        class="font-medium {gamingStatus?.gpu?.vulkan_ready
                          ? 'text-green-400'
                          : 'text-red-400'}"
                      >
                        {gamingStatus?.gpu?.vulkan_ready
                          ? "✓ Ready"
                          : "✗ Not Ready"}
                      </span>
                    </div>
                  </div>
                  {#if !gamingStatus?.gpu?.vulkan_ready}
                    <button
                      class="btn btn-primary w-full mt-4"
                      onclick={handleInstallVulkan}
                    >
                      Install Vulkan Support
                    </button>
                  {/if}
                </div>

                <!-- 32-bit Support -->
                <div class="card">
                  <h3 class="section-title">🔧 32-bit Support (Multilib)</h3>
                  <p class="text-sm text-gray-400 mb-4">
                    Steam and many games need 32-bit libraries. This is
                    essential for gaming on Linux.
                  </p>
                  <div
                    class="flex justify-between p-3 bg-surface-800 rounded-lg"
                  >
                    <span class="text-gray-400">32-bit Libraries</span>
                    <span
                      class="font-medium {gamingStatus?.multilib_enabled
                        ? 'text-green-400'
                        : 'text-red-400'}"
                    >
                      {gamingStatus?.multilib_enabled
                        ? "✓ Enabled"
                        : "✗ Not Enabled"}
                    </span>
                  </div>
                  {#if !gamingStatus?.multilib_enabled}
                    <button
                      class="btn btn-primary w-full mt-4"
                      onclick={handleEnableMultilib}
                    >
                      Enable 32-bit Support
                    </button>
                  {/if}
                </div>
              </div>
            {/if}

            <!-- Tab 3: Performance Tweaks -->
            {#if gamingTab === "tweaks"}
              <div class="card mb-4">
                <div class="flex items-center justify-between">
                  <div>
                    <h3 class="text-lg font-semibold">
                      Apply All Recommended Tweaks
                    </h3>
                    <p class="text-sm text-gray-400">
                      One-click optimization from Nobara Project & AdelKS Guide
                    </p>
                  </div>
                  <button
                    class="btn btn-primary btn-lg"
                    disabled={applyingGamingTweak === "all"}
                    onclick={handleApplyAllGamingTweaks}
                  >
                    {applyingGamingTweak === "all"
                      ? "Applying..."
                      : "⚡ Optimize All"}
                  </button>
                </div>
              </div>

              <div class="space-y-4">
                {#each gamingTweaks as tweak}
                  <div class="card">
                    <div class="flex items-start justify-between">
                      <div class="flex-1">
                        <div class="flex items-center gap-2">
                          <h4 class="font-semibold">{tweak.name}</h4>
                          {#if tweak.risk_level === "safe"}
                            <span
                              class="px-2 py-0.5 text-xs bg-green-500/20 text-green-400 rounded"
                              >Safe</span
                            >
                          {:else if tweak.risk_level === "moderate"}
                            <span
                              class="px-2 py-0.5 text-xs bg-yellow-500/20 text-yellow-400 rounded"
                              >Moderate</span
                            >
                          {:else}
                            <span
                              class="px-2 py-0.5 text-xs bg-red-500/20 text-red-400 rounded"
                              >Advanced</span
                            >
                          {/if}
                          {#if tweak.requires_reboot}
                            <span
                              class="px-2 py-0.5 text-xs bg-purple-500/20 text-purple-400 rounded"
                              >Needs Reboot</span
                            >
                          {/if}
                        </div>
                        <p class="text-sm text-gray-400 mt-1">
                          {tweak.description}
                        </p>

                        <div class="mt-3 flex items-center gap-4">
                          <div class="flex items-center gap-2">
                            <span class="text-xs text-gray-500">Current:</span>
                            <span
                              class="font-mono text-sm {tweak.is_optimal
                                ? 'text-green-400'
                                : 'text-yellow-400'}"
                            >
                              {tweak.current_value}
                            </span>
                          </div>
                          <div class="flex items-center gap-2">
                            <span class="text-xs text-gray-500"
                              >Recommended:</span
                            >
                            <span class="font-mono text-sm text-primary-400"
                              >{tweak.recommended_value}</span
                            >
                          </div>
                        </div>

                        <!-- Slider for numeric tweaks -->
                        {#if tweak.value_type === "slider" && tweak.id !== "vm.max_map_count"}
                          <div class="mt-3">
                            <input
                              type="range"
                              class="w-full accent-primary-500"
                              min={tweak.min_value}
                              max={tweak.max_value}
                              value={tweak.current_value}
                              onchange={(e) =>
                                handleApplyGamingTweak(
                                  tweak.id,
                                  e.currentTarget.value,
                                )}
                            />
                          </div>
                        {/if}
                      </div>

                      <div class="flex items-center gap-2 ml-4">
                        {#if tweak.is_optimal}
                          <span class="text-green-400 text-sm">✓ Optimal</span>
                        {:else}
                          <button
                            class="btn btn-sm btn-primary"
                            disabled={applyingGamingTweak === tweak.id}
                            onclick={() =>
                              handleApplyGamingTweak(
                                tweak.id,
                                tweak.recommended_value,
                              )}
                          >
                            {applyingGamingTweak === tweak.id
                              ? "Applying..."
                              : "Apply"}
                          </button>
                        {/if}
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}

            <!-- Tab 4: One-Touch Setup -->
            {#if gamingTab === "onetouch"}
              <div class="space-y-6">
                <!-- System Profile -->
                {#if systemProfile}
                  <div
                    class="card bg-gradient-to-r from-primary-500/10 to-accent-500/10"
                  >
                    <div class="flex items-center gap-4">
                      <div class="text-5xl">
                        {#if systemProfile.tier === "high"}🚀
                        {:else if systemProfile.tier === "medium"}💪
                        {:else}🎮{/if}
                      </div>
                      <div>
                        <h3 class="text-xl font-bold">
                          {systemProfile.description}
                        </h3>
                        <p class="text-sm text-gray-400 mt-1">
                          Tier: <span class="font-semibold text-primary-400"
                            >{systemProfile.tier.toUpperCase()}</span
                          >
                          • RAM: {systemProfile.ram_gb}GB • CPU: {systemProfile.cpu_cores}
                          cores • GPU: {systemProfile.gpu_vendor?.toUpperCase()}
                        </p>
                      </div>
                    </div>
                  </div>
                {/if}

                <!-- Gaming Checklist -->
                <div class="card">
                  <h3 class="section-title mb-4">
                    📋 Gaming Readiness Checklist
                  </h3>
                  {#if gamingChecklist}
                    <div class="grid grid-cols-2 md:grid-cols-3 gap-3">
                      <div
                        class="flex items-center gap-2 p-3 rounded-lg {gamingChecklist.multilib_ok
                          ? 'bg-green-500/10'
                          : 'bg-red-500/10'}"
                      >
                        <span class="text-xl"
                          >{gamingChecklist.multilib_ok ? "✓" : "✗"}</span
                        >
                        <span class="text-sm">32-bit Support</span>
                      </div>
                      <div
                        class="flex items-center gap-2 p-3 rounded-lg {gamingChecklist.vulkan_ok
                          ? 'bg-green-500/10'
                          : 'bg-red-500/10'}"
                      >
                        <span class="text-xl"
                          >{gamingChecklist.vulkan_ok ? "✓" : "✗"}</span
                        >
                        <span class="text-sm">Vulkan</span>
                      </div>
                      <div
                        class="flex items-center gap-2 p-3 rounded-lg {gamingChecklist.drivers_ok
                          ? 'bg-green-500/10'
                          : 'bg-red-500/10'}"
                      >
                        <span class="text-xl"
                          >{gamingChecklist.drivers_ok ? "✓" : "✗"}</span
                        >
                        <span class="text-sm">GPU Driver</span>
                      </div>
                      <div
                        class="flex items-center gap-2 p-3 rounded-lg {gamingChecklist.kernel_tweaks_ok
                          ? 'bg-green-500/10'
                          : 'bg-red-500/10'}"
                      >
                        <span class="text-xl"
                          >{gamingChecklist.kernel_tweaks_ok ? "✓" : "✗"}</span
                        >
                        <span class="text-sm">Kernel Tweaks</span>
                      </div>
                      <div
                        class="flex items-center gap-2 p-3 rounded-lg {gamingChecklist.limits_ok
                          ? 'bg-green-500/10'
                          : 'bg-red-500/10'}"
                      >
                        <span class="text-xl"
                          >{gamingChecklist.limits_ok ? "✓" : "✗"}</span
                        >
                        <span class="text-sm">ESYNC/FSYNC</span>
                      </div>
                      <div
                        class="flex items-center gap-2 p-3 rounded-lg {gamingChecklist.gamemode_ok
                          ? 'bg-green-500/10'
                          : 'bg-red-500/10'}"
                      >
                        <span class="text-xl"
                          >{gamingChecklist.gamemode_ok ? "✓" : "✗"}</span
                        >
                        <span class="text-sm">GameMode</span>
                      </div>
                    </div>

                    {#if gamingChecklist.missing?.length > 0}
                      <div class="mt-4 p-3 rounded-lg bg-yellow-500/10">
                        <p class="text-sm font-semibold text-yellow-400">
                          ⚠ Issues Found:
                        </p>
                        <ul class="text-sm text-gray-400 mt-2 space-y-1">
                          {#each gamingChecklist.missing as issue}
                            <li>• {issue}</li>
                          {/each}
                        </ul>
                      </div>
                    {/if}
                  {/if}
                </div>

                <!-- One-Touch Action -->
                <div
                  class="card bg-gradient-to-r from-green-500/20 to-primary-500/20 border-green-500/30"
                >
                  <div class="flex items-center justify-between">
                    <div>
                      <h3 class="text-2xl font-bold">
                        ⚡ One-Touch Gaming Setup
                      </h3>
                      <p class="text-gray-400 mt-2">
                        Satu klik untuk install semua yang dibutuhkan gaming di
                        Linux:
                      </p>
                      <ul class="text-sm text-gray-500 mt-2 space-y-1">
                        <li>✓ Layer 1: 32-bit support + GPU Drivers</li>
                        <li>✓ Layer 2: Vulkan + Wine + GameMode</li>
                        <li>✓ Layer 3: Kernel Tweaks + ESYNC</li>
                        <li>✓ Layer 4: Steam + MangoHud + Heroic</li>
                      </ul>
                    </div>
                    <button
                      class="btn btn-lg bg-gradient-to-r from-green-500 to-primary-500 text-white font-bold px-8 py-4 text-lg shadow-lg hover:shadow-green-500/20"
                      disabled={runningOneTouch || gamingChecklist?.all_ok}
                      onclick={handleOneTouchSetup}
                    >
                      {#if runningOneTouch}
                        <div class="spinner w-5 h-5 mr-2"></div>
                        Installing...
                      {:else if gamingChecklist?.all_ok}
                        ✓ Already Optimized!
                      {:else}
                        🚀 Setup Gaming Now!
                      {/if}
                    </button>
                  </div>
                </div>

                <!-- Installation Logs -->
                {#if oneTouchLogs.length > 0}
                  <div class="card">
                    <h3 class="section-title mb-3">📜 Installation Log</h3>
                    <div
                      class="bg-surface-900 rounded-lg p-4 font-mono text-sm max-h-64 overflow-y-auto"
                    >
                      {#each oneTouchLogs as log}
                        <p
                          class={log.startsWith("✓")
                            ? "text-green-400"
                            : log.startsWith("⚠") || log.startsWith("❌")
                              ? "text-yellow-400"
                              : "text-gray-400"}
                        >
                          {log}
                        </p>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            {/if}
          {/if}
        </div>
      {:else if currentPage === "about"}
        <!-- About Page -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <!-- Left Column -->
          <div class="space-y-6">
            <!-- App Header Card -->
            <div class="card">
              <div class="flex items-center gap-6">
                <div
                  class="w-20 h-20 rounded-2xl bg-gradient-to-br from-primary-500 to-accent-500 flex items-center justify-center text-4xl font-bold text-white shadow-lg"
                >
                  G
                </div>
                <div>
                  <h1 class="text-2xl font-bold">Glance</h1>
                  <p class="text-gray-400">Version 26.1.1</p>
                  <p class="text-sm text-gray-500 mt-1">
                    Linux System Optimizer
                  </p>
                </div>
              </div>
            </div>

            <!-- Description Card -->
            <div class="card">
              <h3 class="section-title">About</h3>
              <p class="text-gray-400 leading-relaxed">
                A next-generation Linux system utility that combines monitoring,
                cleaning, and optimization into one stunning application. Built
                with modern technologies for a native-like experience.
              </p>
              <p class="text-gradient font-semibold mt-4">
                See Everything. Optimize Anything. Beautiful by Default.
              </p>
            </div>

            <!-- Credits Card -->
            <div class="card">
              <h3 class="section-title">Credits</h3>
              <div class="space-y-2">
                <div
                  class="flex items-center justify-between p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-gray-400">Developer</span>
                  <span class="font-medium">WRVbit</span>
                </div>
                <div
                  class="flex items-center justify-between p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-gray-400">License</span>
                  <span class="font-medium">GPL-3.0</span>
                </div>
                <div
                  class="flex items-center justify-between p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-gray-400">Year</span>
                  <span class="font-medium">2025</span>
                </div>
                <div
                  class="flex items-center justify-between p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-gray-400">Repository</span>
                  <span class="text-primary-400">github.com/WRVbit/glance</span>
                </div>
              </div>
            </div>
          </div>

          <!-- Right Column -->
          <div class="space-y-6">
            <!-- Tech Stack Card -->
            <div class="card">
              <h3 class="section-title">Built With</h3>
              <div class="grid grid-cols-2 gap-3">
                <div
                  class="flex flex-col items-center p-4 rounded-xl bg-surface-800/50"
                >
                  <span class="text-3xl mb-2">🦀</span>
                  <span class="font-medium">Rust</span>
                  <span class="text-xs text-gray-500">Backend</span>
                </div>
                <div
                  class="flex flex-col items-center p-4 rounded-xl bg-surface-800/50"
                >
                  <span class="text-3xl mb-2">🔥</span>
                  <span class="font-medium">Svelte 5</span>
                  <span class="text-xs text-gray-500">Frontend</span>
                </div>
                <div
                  class="flex flex-col items-center p-4 rounded-xl bg-surface-800/50"
                >
                  <span class="text-3xl mb-2">⚡</span>
                  <span class="font-medium">Tauri 2</span>
                  <span class="text-xs text-gray-500">Framework</span>
                </div>
                <div
                  class="flex flex-col items-center p-4 rounded-xl bg-surface-800/50"
                >
                  <span class="text-3xl mb-2">🎨</span>
                  <span class="font-medium">Glassmorphism</span>
                  <span class="text-xs text-gray-500">Design</span>
                </div>
              </div>
            </div>

            <!-- Features Card -->
            <div class="card">
              <h3 class="section-title">Features</h3>
              <div class="grid grid-cols-1 gap-2">
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Real-time System Monitoring</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Disk Space Cleaner</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Performance Tweaks</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Package Management</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Service Control</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Repository Manager</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>DNS-level Ad Blocking</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-green-400">✓</span>
                  <span>Gaming Center (One-Touch Setup)</span>
                </div>
                <div
                  class="flex items-center gap-3 p-3 rounded-lg bg-surface-800/50"
                >
                  <span class="text-primary-400">✓</span>
                  <span>Multi-distro Support</span>
                </div>
              </div>
            </div>

            <!-- Thank You Card -->
            <div class="card text-center py-6">
              <p class="text-gray-400">Thank you for using Glance,</p>
              <p class="text-xl font-bold text-gradient mt-1">
                {systemInfo?.hostname ?? "User"}!
              </p>
              <p class="text-sm text-gray-500 mt-2">
                Made with ❤️ for the Linux community
              </p>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </main>
</div>
