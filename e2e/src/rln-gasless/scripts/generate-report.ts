#!/usr/bin/env ts-node
/**
 * RLN Gasless E2E Test Report Generator
 *
 * This script parses Jest JSON output and generates a business-focused
 * scenario report. It extracts scenario IDs and descriptions from test names.
 *
 * Usage:
 *   npx ts-node src/rln-gasless/scripts/generate-report.ts [input] [output]
 *
 * Example:
 *   npx ts-node src/rln-gasless/scripts/generate-report.ts jest-results.json rln-gasless-report.json
 */

import * as fs from "fs";
import * as path from "path";
import { ALL_SCENARIOS, ScenarioCategory } from "../helpers/scenario";

// Types for Jest JSON output
interface JestAssertionResult {
  ancestorTitles: string[];
  fullName: string;
  status: "passed" | "failed" | "pending" | "skipped";
  title: string;
  duration: number | null;
  failureMessages: string[];
}

interface JestTestResult {
  assertionResults: JestAssertionResult[];
  endTime: number;
  startTime: number;
  name: string;
  status: "passed" | "failed";
}

interface JestOutput {
  numFailedTestSuites: number;
  numPassedTestSuites: number;
  numTotalTestSuites: number;
  numFailedTests: number;
  numPassedTests: number;
  numPendingTests: number;
  numTotalTests: number;
  testResults: JestTestResult[];
  startTime: number;
  success: boolean;
}

// Report types
interface ScenarioResult {
  scenario_id: string;
  description: string;
  category: ScenarioCategory | "UNKNOWN";
  status: "passed" | "failed" | "pending" | "skipped" | "not_run";
  duration_ms: number | null;
  failure_message?: string;
  test_file?: string;
}

interface CategorySummary {
  category: string;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  not_run: number;
  pass_rate: string;
}

interface ReportSummary {
  generated_at: string;
  test_run_start: string;
  test_run_end: string;
  total_duration_ms: number;
  total_scenarios: number;
  passed: number;
  failed: number;
  skipped: number;
  not_run: number;
  pass_rate: string;
  categories: CategorySummary[];
}

interface BusinessReport {
  summary: ReportSummary;
  scenarios: ScenarioResult[];
  failed_scenarios: ScenarioResult[];
}

// Scenario ID pattern: [SCENARIO_ID] Description
const SCENARIO_PATTERN = /\[([A-Z]+_\d+)\]\s+(.+)/;

function parseScenarioFromTestName(fullName: string): { id: string; description: string } | null {
  const match = fullName.match(SCENARIO_PATTERN);
  if (match) {
    return {
      id: match[1],
      description: match[2],
    };
  }
  return null;
}

function getCategoryForScenario(scenarioId: string): ScenarioCategory | "UNKNOWN" {
  const scenario = ALL_SCENARIOS.find((s) => s.id === scenarioId);
  return scenario?.category ?? "UNKNOWN";
}

function extractTestFile(filePath: string): string {
  const basename = path.basename(filePath);
  return basename.replace(".spec.ts", "").replace(".spec.js", "");
}

function generateReport(jestOutput: JestOutput): BusinessReport {
  const scenarioResults: Map<string, ScenarioResult> = new Map();

  // Initialize all known scenarios as "not_run"
  for (const scenario of ALL_SCENARIOS) {
    scenarioResults.set(scenario.id, {
      scenario_id: scenario.id,
      description: scenario.description,
      category: scenario.category,
      status: "not_run",
      duration_ms: null,
    });
  }

  // Process test results
  for (const testFile of jestOutput.testResults) {
    const fileName = extractTestFile(testFile.name);

    for (const assertion of testFile.assertionResults) {
      const parsed = parseScenarioFromTestName(assertion.title);

      if (parsed) {
        const result: ScenarioResult = {
          scenario_id: parsed.id,
          description: parsed.description,
          category: getCategoryForScenario(parsed.id),
          status: assertion.status,
          duration_ms: assertion.duration,
          test_file: fileName,
        };

        if (assertion.status === "failed" && assertion.failureMessages.length > 0) {
          result.failure_message = assertion.failureMessages[0].split("\n")[0];
        }

        scenarioResults.set(parsed.id, result);
      }
    }
  }

  const results = Array.from(scenarioResults.values());

  // Calculate summary
  const passed = results.filter((r) => r.status === "passed").length;
  const failed = results.filter((r) => r.status === "failed").length;
  const skipped = results.filter((r) => r.status === "pending" || r.status === "skipped").length;
  const notRun = results.filter((r) => r.status === "not_run").length;
  const total = results.length;

  // Calculate category summaries
  const categoryMap = new Map<string, ScenarioResult[]>();
  for (const result of results) {
    const cat = result.category;
    if (!categoryMap.has(cat)) {
      categoryMap.set(cat, []);
    }
    categoryMap.get(cat)!.push(result);
  }

  const categories: CategorySummary[] = Array.from(categoryMap.entries())
    .map(([category, scenarios]) => {
      const catPassed = scenarios.filter((s) => s.status === "passed").length;
      const catFailed = scenarios.filter((s) => s.status === "failed").length;
      const catSkipped = scenarios.filter((s) => s.status === "pending" || s.status === "skipped").length;
      const catNotRun = scenarios.filter((s) => s.status === "not_run").length;
      const catTotal = scenarios.length;
      const executed = catTotal - catNotRun;

      return {
        category,
        total: catTotal,
        passed: catPassed,
        failed: catFailed,
        skipped: catSkipped,
        not_run: catNotRun,
        pass_rate: executed > 0 ? `${((catPassed / executed) * 100).toFixed(1)}%` : "N/A",
      };
    })
    .sort((a, b) => a.category.localeCompare(b.category));

  // Calculate test run times
  const testStartTime = jestOutput.startTime;
  let testEndTime = testStartTime;
  for (const testFile of jestOutput.testResults) {
    if (testFile.endTime > testEndTime) {
      testEndTime = testFile.endTime;
    }
  }

  const executed = total - notRun;

  const summary: ReportSummary = {
    generated_at: new Date().toISOString(),
    test_run_start: new Date(testStartTime).toISOString(),
    test_run_end: new Date(testEndTime).toISOString(),
    total_duration_ms: testEndTime - testStartTime,
    total_scenarios: total,
    passed,
    failed,
    skipped,
    not_run: notRun,
    pass_rate: executed > 0 ? `${((passed / executed) * 100).toFixed(1)}%` : "N/A",
    categories,
  };

  // Sort results by scenario ID
  results.sort((a, b) => a.scenario_id.localeCompare(b.scenario_id));

  const failedScenarios = results.filter((r) => r.status === "failed");

  return {
    summary,
    scenarios: results,
    failed_scenarios: failedScenarios,
  };
}

function generateMarkdownReport(report: BusinessReport): string {
  const lines: string[] = [];

  lines.push("# RLN Gasless E2E Test Report");
  lines.push("");
  lines.push(`**Generated:** ${report.summary.generated_at}`);
  lines.push("");

  // Summary
  lines.push("## Summary");
  lines.push("");
  lines.push("| Metric | Value |");
  lines.push("|--------|-------|");
  lines.push(`| Total Scenarios | ${report.summary.total_scenarios} |`);
  lines.push(`| Passed | ${report.summary.passed} |`);
  lines.push(`| Failed | ${report.summary.failed} |`);
  lines.push(`| Skipped | ${report.summary.skipped} |`);
  lines.push(`| Not Run | ${report.summary.not_run} |`);
  lines.push(`| Pass Rate | ${report.summary.pass_rate} |`);
  lines.push(`| Duration | ${(report.summary.total_duration_ms / 1000).toFixed(1)}s |`);
  lines.push("");

  // Category breakdown
  lines.push("## Category Breakdown");
  lines.push("");
  lines.push("| Category | Total | Passed | Failed | Skipped | Pass Rate |");
  lines.push("|----------|-------|--------|--------|---------|-----------|");

  for (const cat of report.summary.categories) {
    lines.push(
      `| ${cat.category} | ${cat.total} | ${cat.passed} | ${cat.failed} | ${cat.skipped} | ${cat.pass_rate} |`,
    );
  }
  lines.push("");

  // Failed scenarios (if any)
  if (report.failed_scenarios.length > 0) {
    lines.push("## ❌ Failed Scenarios");
    lines.push("");

    for (const scenario of report.failed_scenarios) {
      lines.push(`### ${scenario.scenario_id}: ${scenario.description}`);
      lines.push("");
      lines.push(`- **Category:** ${scenario.category}`);
      lines.push(`- **Test File:** ${scenario.test_file}`);
      if (scenario.failure_message) {
        lines.push(`- **Error:** \`${scenario.failure_message}\``);
      }
      lines.push("");
    }
  }

  // All scenarios by category
  lines.push("## Scenario Details");
  lines.push("");

  const categorized = new Map<string, ScenarioResult[]>();
  for (const scenario of report.scenarios) {
    const cat = scenario.category;
    if (!categorized.has(cat)) {
      categorized.set(cat, []);
    }
    categorized.get(cat)!.push(scenario);
  }

  const sortedCategories = Array.from(categorized.keys()).sort();

  for (const category of sortedCategories) {
    const scenarios = categorized.get(category)!;
    lines.push(`### ${category}`);
    lines.push("");
    lines.push("| ID | Description | Status | Duration |");
    lines.push("|----|-------------|--------|----------|");

    for (const s of scenarios) {
      const statusIcon =
        s.status === "passed" ? "✅" : s.status === "failed" ? "❌" : s.status === "not_run" ? "⏸️" : "⏭️";
      const duration = s.duration_ms !== null ? `${s.duration_ms}ms` : "-";
      lines.push(`| ${s.scenario_id} | ${s.description} | ${statusIcon} ${s.status} | ${duration} |`);
    }
    lines.push("");
  }

  return lines.join("\n");
}

function main() {
  const args = process.argv.slice(2);

  const inputFile = args[0] || "jest-results.json";
  const outputFile = args[1] || "rln-gasless-report.json";

  // Read Jest output
  if (!fs.existsSync(inputFile)) {
    console.error(`Error: Input file '${inputFile}' not found.`);
    console.error("Run tests with: npm run test:rln-gasless:json");
    process.exit(1);
  }

  console.log(`Reading Jest results from: ${inputFile}`);
  const jestData = JSON.parse(fs.readFileSync(inputFile, "utf8")) as JestOutput;

  // Generate report
  console.log("Generating business report...");
  const report = generateReport(jestData);

  // Write JSON report
  fs.writeFileSync(outputFile, JSON.stringify(report, null, 2));
  console.log(`JSON report written to: ${outputFile}`);

  // Write Markdown report
  const mdOutputFile = outputFile.replace(".json", ".md");
  const markdownReport = generateMarkdownReport(report);
  fs.writeFileSync(mdOutputFile, markdownReport);
  console.log(`Markdown report written to: ${mdOutputFile}`);

  // Print summary to console
  console.log("\n" + "=".repeat(60));
  console.log("RLN GASLESS E2E TEST REPORT SUMMARY");
  console.log("=".repeat(60));
  console.log(`Total Scenarios: ${report.summary.total_scenarios}`);
  console.log(`Passed:          ${report.summary.passed} ✅`);
  console.log(`Failed:          ${report.summary.failed} ❌`);
  console.log(`Skipped:         ${report.summary.skipped} ⏭️`);
  console.log(`Not Run:         ${report.summary.not_run} ⏸️`);
  console.log(`Pass Rate:       ${report.summary.pass_rate}`);
  console.log(`Duration:        ${(report.summary.total_duration_ms / 1000).toFixed(1)}s`);
  console.log("=".repeat(60));

  if (report.failed_scenarios.length > 0) {
    console.log("\n❌ FAILED SCENARIOS:");
    for (const s of report.failed_scenarios) {
      console.log(`  - [${s.scenario_id}] ${s.description}`);
      if (s.failure_message) {
        console.log(`    Error: ${s.failure_message}`);
      }
    }
  }

  // Exit with appropriate code
  if (report.summary.failed > 0) {
    process.exit(1);
  }
}

main();
