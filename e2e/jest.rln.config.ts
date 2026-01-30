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
};

export default config;
