import type { Config } from "jest";

const config: Config = {
  displayName: "e2e-rln",
  preset: "ts-jest",
  testEnvironment: "node",
  testTimeout: 300_000,
  slowTestThreshold: 100,
  testMatch: ["**/src/rln-gasless/**/*.spec.ts"],
  modulePathIgnorePatterns: ["<rootDir>/src/typechain/"],
  setupFiles: ["<rootDir>/src/config/jest/setup.ts"],
  // Skip global setup/teardown for RLN tests - they have their own initialization
  // globalSetup: "<rootDir>/src/config/jest/global-setup.ts",
  // globalTeardown: "<rootDir>/src/config/jest/global-teardown.ts",
  reporters: ["default"],
  // Run test suites sequentially — all suites share the same admin wallet
  // for funding/registration, so parallel execution causes nonce race conditions.
  maxWorkers: 1,
};

export default config;
