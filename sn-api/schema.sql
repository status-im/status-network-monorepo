-- SN-API Schema for Replicated Tables
-- This schema mimics the rln-prover database tables that are replicated.

CREATE TABLE IF NOT EXISTS tx_counter (
    id BIGSERIAL NOT NULL PRIMARY KEY,
    address TEXT UNIQUE,
    epoch BIGINT DEFAULT 0,
    epoch_counter BIGINT DEFAULT 0
);

CREATE TABLE IF NOT EXISTS deny_list (
    address CHAR(42) NOT NULL PRIMARY KEY,
    expires_at BIGINT,
    denied_at BIGINT
);
