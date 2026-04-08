import type { Config } from "jest";

const config: Config = {
  displayName: "e2e-rln-slashing",
  preset: "ts-jest",
  testEnvironment: "node",
  testTimeout: 300_000,
  slowTestThreshold: 100,
  testMatch: ["**/src/rln-slashing/**/*.spec.ts"],
  modulePathIgnorePatterns: ["<rootDir>/src/typechain/"],
  setupFiles: ["<rootDir>/src/config/jest/setup.ts"],
  reporters: ["default"],
  // Sequential execution: failover tests stop/start containers, so parallel
  // execution would race with each other (and with the rln-gasless suite if
  // run together).
  maxWorkers: 1,
};

export default config;
