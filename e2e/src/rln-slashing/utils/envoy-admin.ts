/**
 * Thin wrapper around the Envoy admin interface.
 *
 * Envoy exposes an HTTP admin endpoint (configured in
 * docker/config/envoy/envoy.yaml on :9901) that we use for read-only health
 * and stats inspection in tests:
 *
 *   GET /ready              -> "LIVE\n" when up
 *   GET /clusters           -> per-backend stats and health flags (text)
 *   GET /clusters?format=json -> same as JSON
 *   GET /stats              -> all counters (text)
 *   GET /listeners          -> listener state
 *
 * We deliberately do NOT use a typed Envoy admin client because the surface
 * we need is tiny and the JSON format is stable enough.
 */

import { SLASHING_CONFIG } from "../config/rln-slashing-config";

export interface BackendHealth {
  /** "host:port" of the backend (e.g. "11.11.11.123:50061") */
  address: string;
  /** Empty string when healthy. Otherwise space-separated flags from Envoy. */
  healthFlagsText: string;
  /** True iff Envoy reports this backend as healthy (no flags set). */
  isHealthy: boolean;
  /** Number of active connections from Envoy to this backend. */
  cxActive: number;
  /** Number of active requests in flight to this backend. */
  rqActive: number;
  /** Total requests routed to this backend since Envoy startup. */
  rqTotal: number;
  /** Hostname as seen by Envoy (if it resolved STRICT_DNS to a hostname). */
  hostname: string;
}

export interface ClusterHealth {
  /** Cluster name (e.g. "rln_aggregators"). */
  name: string;
  /** Per-backend stats. */
  backends: BackendHealth[];
  /** Total healthy backends in the cluster. */
  healthyCount: number;
  /** Total backends in the cluster (healthy + unhealthy). */
  totalCount: number;
}

const adminUrl = (path: string) => `${SLASHING_CONFIG.envoy.adminBaseUrl}${path}`;

async function envoyGet(path: string): Promise<string> {
  const url = adminUrl(path);
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`Envoy admin GET ${path} returned HTTP ${res.status}`);
  }
  return res.text();
}

/**
 * Returns "LIVE" / "PRE_INITIALIZING" / etc. as reported by Envoy /ready.
 * Throws if the admin endpoint is unreachable.
 */
export async function getEnvoyReadyState(): Promise<string> {
  return (await envoyGet("/ready")).trim();
}

/**
 * Parses the text output of /clusters into per-backend stats.
 *
 * The /clusters text output uses lines like:
 *   <cluster>::<address>::<key>::<value>
 *
 * Example:
 *   rln_aggregators::11.11.11.123:50061::cx_active::0
 *   rln_aggregators::11.11.11.123:50061::health_flags::healthy
 *   rln_aggregators::11.11.11.123:50061::hostname::rln-aggregator-1
 *
 * We only need a small subset of the keys per backend; everything else is
 * ignored. Backends are identified by their host:port substring.
 */
export async function getClusterHealth(
  clusterName: string = SLASHING_CONFIG.envoy.clusterName,
): Promise<ClusterHealth> {
  const text = await envoyGet("/clusters");
  const prefix = `${clusterName}::`;

  // address -> { key -> value }
  const perBackend = new Map<string, Record<string, string>>();
  for (const line of text.split("\n")) {
    if (!line.startsWith(prefix)) continue;
    const rest = line.slice(prefix.length);
    // rest is like "11.11.11.123:50061::health_flags::healthy" OR "version::0"
    // (the latter is per-cluster, not per-backend, and has no host:port).
    const parts = rest.split("::");
    if (parts.length < 3) continue;
    const address = parts[0];
    if (!address.includes(":")) continue; // skip per-cluster lines
    const key = parts[1];
    const value = parts.slice(2).join("::");

    if (!perBackend.has(address)) perBackend.set(address, {});
    perBackend.get(address)![key] = value;
  }

  const backends: BackendHealth[] = [];
  for (const [address, kv] of perBackend.entries()) {
    const healthFlagsText = kv["health_flags"] || "";
    backends.push({
      address,
      healthFlagsText,
      isHealthy: healthFlagsText === "healthy",
      cxActive: parseInt(kv["cx_active"] || "0", 10),
      rqActive: parseInt(kv["rq_active"] || "0", 10),
      rqTotal: parseInt(kv["rq_total"] || "0", 10),
      hostname: kv["hostname"] || "",
    });
  }
  // Sort for stable test assertions
  backends.sort((a, b) => a.address.localeCompare(b.address));

  return {
    name: clusterName,
    backends,
    healthyCount: backends.filter((b) => b.isHealthy).length,
    totalCount: backends.length,
  };
}

/**
 * Polls Envoy /clusters until a predicate over the cluster state is satisfied,
 * or the timeout is reached. Throws on timeout with the last observed state.
 *
 * Used for asserting on transitions (e.g., a backend going from healthy ->
 * unhealthy after a docker stop).
 */
export async function waitForClusterState(
  predicate: (c: ClusterHealth) => boolean,
  options: { timeoutMs?: number; pollIntervalMs?: number; clusterName?: string; description?: string } = {},
): Promise<ClusterHealth> {
  const timeoutMs = options.timeoutMs ?? SLASHING_CONFIG.timeouts.envoyHealthFlipMs;
  const pollIntervalMs = options.pollIntervalMs ?? SLASHING_CONFIG.timeouts.pollIntervalMs;
  const clusterName = options.clusterName ?? SLASHING_CONFIG.envoy.clusterName;
  const description = options.description ?? "cluster predicate";

  const deadline = Date.now() + timeoutMs;
  let last: ClusterHealth | undefined;
  let lastError: unknown;
  while (Date.now() < deadline) {
    try {
      last = await getClusterHealth(clusterName);
      if (predicate(last)) return last;
    } catch (e) {
      lastError = e;
    }
    await new Promise((r) => setTimeout(r, pollIntervalMs));
  }
  throw new Error(
    `waitForClusterState timed out after ${timeoutMs}ms waiting for ${description}. ` +
      `Last state: ${last ? JSON.stringify(last, null, 2) : "(unreachable)"}` +
      (lastError ? `, last error: ${lastError instanceof Error ? lastError.message : String(lastError)}` : ""),
  );
}

/**
 * Returns the value of a single Envoy stat counter (or undefined if not present).
 * Stat keys look like "cluster.rln_aggregators.upstream_rq_total".
 */
export async function getEnvoyStat(statKey: string): Promise<number | undefined> {
  const text = await envoyGet(`/stats?filter=^${statKey}$`);
  for (const line of text.split("\n")) {
    if (line.startsWith(`${statKey}:`)) {
      const value = line.slice(statKey.length + 1).trim();
      const n = parseInt(value, 10);
      return Number.isNaN(n) ? undefined : n;
    }
  }
  return undefined;
}
