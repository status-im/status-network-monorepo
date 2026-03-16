//! Migration tool to update user_limit (spam_limit) for all registered RLN users.
//!
//! When --spam-limit is changed on the prover, existing users retain the old value
//! in their stored identity and Merkle tree leaf. This tool updates both so that
//! proof generation uses the new limit without re-registration.
//!
//! IMPORTANT: Stop the RLN prover before running this migration.

use ark_bn254::Fr;
use ark_serialize::CanonicalSerialize;
use clap::Parser;
use rln::hashers::poseidon_hash;
use rln_proof::RlnUserIdentity;
use sqlx::{PgPool, Row};

#[derive(Parser)]
#[command(name = "migrate-spam-limit")]
#[command(
    about = "Update user_limit for all registered RLN users and recompute Merkle tree leaves"
)]
struct Args {
    /// PostgreSQL connection URL
    #[arg(long, env = "DATABASE_URL")]
    db: String,

    /// New spam limit value (max: 1048575 due to circuit Num2Bits(20) constraint)
    #[arg(long)]
    new_limit: u64,

    /// Merkle tree depth (default: 20, matches prover default)
    #[arg(long, default_value = "20")]
    tree_depth: i16,

    /// Dry run — show what would change without writing
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("=== RLN Spam Limit Migration ===");
    println!("Database:   {}", redact_password(&args.db));
    println!("New limit:  {}", args.new_limit);
    println!("Tree depth: {}", args.tree_depth);
    println!("Dry run:    {}", args.dry_run);
    println!();

    let pool = match PgPool::connect(&args.db).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("ERROR: Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = run_migration(&pool, &args).await {
        eprintln!("ERROR: Migration failed: {}", e);
        std::process::exit(1);
    }
}

async fn run_migration(pool: &PgPool, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let new_limit_fr = Fr::from(args.new_limit);

    // Read all users
    let rows =
        sqlx::query("SELECT id, rln_id, tree_index, index_in_merkle_tree FROM users ORDER BY id")
            .fetch_all(pool)
            .await?;

    println!("Found {} registered users", rows.len());
    if rows.is_empty() {
        println!("Nothing to migrate.");
        return Ok(());
    }
    println!();

    let mut updated = 0u64;
    let mut skipped = 0u64;
    let mut errors = 0u64;

    for row in &rows {
        let id: i64 = row.get("id");
        let rln_id: serde_json::Value = row.get("rln_id");
        let tree_index: i64 = row.get("tree_index");
        let leaf_index: i64 = row.get("index_in_merkle_tree");

        // Deserialize identity
        let identity: RlnUserIdentity = match serde_json::from_value(rln_id.clone()) {
            Ok(id) => id,
            Err(e) => {
                eprintln!(
                    "  User {} (tree={}, leaf={}): FAILED to deserialize identity: {}",
                    id, tree_index, leaf_index, e
                );
                errors += 1;
                continue;
            }
        };

        // Check if already at the correct limit
        if identity.user_limit == new_limit_fr {
            println!(
                "  User {} (tree={}, leaf={}): already at limit {}, skipping",
                id, tree_index, leaf_index, args.new_limit
            );
            skipped += 1;
            continue;
        }

        // Build updated identity
        let new_identity =
            RlnUserIdentity::from((identity.commitment, identity.secret_hash, new_limit_fr));

        // Compute new rate_commit = poseidon_hash([commitment, new_user_limit])
        let new_rate_commit = poseidon_hash(&[identity.commitment, new_limit_fr]);

        // Serialize rate_commit to 32-byte compressed form for pgfr
        let mut rate_commit_bytes = Vec::with_capacity(32);
        new_rate_commit
            .serialize_compressed(&mut rate_commit_bytes)
            .expect("Fr serialization cannot fail");

        // Serialize new identity to JSON
        let new_rln_id_json = serde_json::to_value(&new_identity)?;

        if args.dry_run {
            println!(
                "  User {} (tree={}, leaf={}): WOULD update user_limit to {}",
                id, tree_index, leaf_index, args.new_limit
            );
        } else {
            // Update in a transaction
            let mut txn = pool.begin().await?;

            // Step 1: Update the Merkle tree leaf with new rate_commit
            // bytea_to_pgfr() casts the 32-byte compressed Fr to the pgfr type
            sqlx::query("SELECT pgfr_mtree_set_leaf($1, $2, $3, bytea_to_pgfr($4))")
                .bind(args.tree_depth)
                .bind(tree_index)
                .bind(leaf_index)
                .bind(&rate_commit_bytes)
                .execute(&mut *txn)
                .await?;

            // Step 2: Update the rln_id JSON in users table
            sqlx::query("UPDATE users SET rln_id = $1 WHERE id = $2")
                .bind(&new_rln_id_json)
                .bind(id)
                .execute(&mut *txn)
                .await?;

            txn.commit().await?;

            println!(
                "  User {} (tree={}, leaf={}): updated",
                id, tree_index, leaf_index
            );
        }

        updated += 1;
    }

    println!();
    println!("=== Migration Summary ===");
    println!("Updated: {}", updated);
    println!("Skipped: {}", skipped);
    println!("Errors:  {}", errors);

    if errors > 0 {
        eprintln!(
            "\nWARNING: {} users failed to migrate. Check errors above.",
            errors
        );
    }

    if args.dry_run {
        println!("\nDry run complete. No changes were made. Re-run without --dry-run to apply.");
    } else {
        println!(
            "\nMigration applied. Restart the RLN prover with --spam-limit {}",
            args.new_limit
        );
    }

    Ok(())
}

/// Redact password from a database URL for logging
fn redact_password(url: &str) -> String {
    if let Some(at_pos) = url.find('@')
        && let Some(colon_pos) = url[..at_pos].rfind(':')
    {
        let scheme_end = url.find("://").map(|p| p + 3).unwrap_or(0);
        if colon_pos > scheme_end {
            return format!("{}:****{}", &url[..colon_pos], &url[at_pos..]);
        }
    }
    url.to_string()
}
