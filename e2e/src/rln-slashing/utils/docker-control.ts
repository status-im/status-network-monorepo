/**
 * Minimal Docker container control wrappers used by failover tests.
 *
 * IMPORTANT — design rationale:
 *
 * The user has an explicit project rule that the L2 stack should always be
 * brought up via `make start-env-with-rln-production` and never piecewise.
 * Partial `docker compose up` of subsets has caused state corruption (silent
 * mock-mode regression) in this project before.
 *
 * The failover tests below ONLY use `docker stop` / `docker start` on existing
 * containers. They do NOT recreate containers, do NOT use `docker compose up`,
 * and do NOT touch any environment variables. `docker stop` followed by
 * `docker start` preserves the container's existing config-hash and command,
 * so the production-mode rln-prover (and any other service) is unaffected.
 *
 * The failover tests are also strictly self-cleaning: every test that stops
 * a container restarts it before returning, and waits for it to become
 * healthy again. If a test crashes mid-way, the afterEach hook restarts the
 * container as a safety net.
 */

import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);

/**
 * Stops a docker container by name. Idempotent: ignores "not running" errors.
 */
export async function dockerStop(container: string): Promise<void> {
  try {
    await execAsync(`docker stop ${container}`);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("No such container") || msg.includes("is not running")) return;
    throw new Error(`docker stop ${container} failed: ${msg}`);
  }
}

/**
 * Starts a previously-created docker container by name. Idempotent: ignores
 * "already started" errors.
 */
export async function dockerStart(container: string): Promise<void> {
  try {
    await execAsync(`docker start ${container}`);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("No such container") || msg.includes("already started")) return;
    throw new Error(`docker start ${container} failed: ${msg}`);
  }
}

/** "running" / "exited" / "created" / "restarting" / etc., or "" if not present. */
export async function getContainerState(container: string): Promise<string> {
  try {
    const { stdout } = await execAsync(`docker inspect --format '{{.State.Status}}' ${container}`);
    return stdout.trim();
  } catch {
    return "";
  }
}

/** "healthy" / "unhealthy" / "starting" / "none" — empty string if not present. */
export async function getContainerHealth(container: string): Promise<string> {
  try {
    const { stdout } = await execAsync(
      `docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' ${container}`,
    );
    return stdout.trim();
  } catch {
    return "";
  }
}

/**
 * Polls until the container's healthcheck reports "healthy" (or returns
 * "none" if the container has no healthcheck configured). Throws on timeout.
 */
export async function waitForContainerHealthy(
  container: string,
  options: { timeoutMs?: number; pollIntervalMs?: number } = {},
): Promise<void> {
  const timeoutMs = options.timeoutMs ?? 60_000;
  const pollIntervalMs = options.pollIntervalMs ?? 500;
  const deadline = Date.now() + timeoutMs;
  let lastState = "";
  while (Date.now() < deadline) {
    lastState = await getContainerHealth(container);
    if (lastState === "healthy" || lastState === "none") return;
    await new Promise((r) => setTimeout(r, pollIntervalMs));
  }
  throw new Error(`waitForContainerHealthy(${container}) timed out after ${timeoutMs}ms. Last state: ${lastState}`);
}
