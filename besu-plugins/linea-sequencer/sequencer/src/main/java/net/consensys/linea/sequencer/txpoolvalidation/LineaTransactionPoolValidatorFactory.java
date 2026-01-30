/*
 * Copyright Consensys Software Inc.
 *
 * This file is dual-licensed under either the MIT license or Apache License 2.0.
 * See the LICENSE-MIT and LICENSE-APACHE files in the repository root for details.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

package net.consensys.linea.sequencer.txpoolvalidation;

import java.util.ArrayList;
import java.util.Optional;
import java.util.Set;
import java.util.concurrent.atomic.AtomicReference;
import net.consensys.linea.config.LineaNodeType;
import net.consensys.linea.config.LineaProfitabilityConfiguration;
import net.consensys.linea.config.LineaRlnValidatorConfiguration;
import net.consensys.linea.config.LineaTracerConfiguration;
import net.consensys.linea.config.LineaTransactionPoolValidatorConfiguration;
import net.consensys.linea.jsonrpc.JsonRpcManager;
import net.consensys.linea.plugins.config.LineaL1L2BridgeSharedConfiguration;
import net.consensys.linea.sequencer.txpoolvalidation.shared.SharedServiceManager;
import net.consensys.linea.sequencer.txpoolvalidation.validators.AllowedAddressValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.CalldataValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.GasLimitValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.ProfitabilityValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.RlnProverForwarderValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.RlnVerifierValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.SimulationValidator;
import net.consensys.linea.sequencer.txpoolvalidation.validators.TraceLineLimitValidator;
import net.consensys.linea.sequencer.txselection.InvalidTransactionByLineCountCache;
import org.hyperledger.besu.datatypes.Address;
import org.hyperledger.besu.plugin.services.BesuConfiguration;
import org.hyperledger.besu.plugin.services.BlockchainService;
import org.hyperledger.besu.plugin.services.TransactionSimulationService;
import org.hyperledger.besu.plugin.services.WorldStateService;
import org.hyperledger.besu.plugin.services.txvalidator.PluginTransactionPoolValidator;
import org.hyperledger.besu.plugin.services.txvalidator.PluginTransactionPoolValidatorFactory;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/** Represents a factory for creating transaction pool validators. */
public class LineaTransactionPoolValidatorFactory implements PluginTransactionPoolValidatorFactory {
  private static final Logger LOG =
      LoggerFactory.getLogger(LineaTransactionPoolValidatorFactory.class);

  private final BesuConfiguration besuConfiguration;
  private final BlockchainService blockchainService;
  private final WorldStateService worldStateService;
  private final TransactionSimulationService transactionSimulationService;
  private final LineaTransactionPoolValidatorConfiguration txPoolValidatorConf;
  private final LineaProfitabilityConfiguration profitabilityConf;
  private final LineaL1L2BridgeSharedConfiguration l1L2BridgeConfiguration;
  private final LineaTracerConfiguration tracerConfiguration;
  private final Optional<JsonRpcManager> rejectedTxJsonRpcManager;
  private final LineaRlnValidatorConfiguration rlnValidatorConf;
  private final SharedServiceManager sharedServiceManager;
  private final boolean rlnProverForwarderEnabled;
  private final InvalidTransactionByLineCountCache invalidTransactionByLineCountCache;
  private final LineaNodeType nodeType;

  private final AtomicReference<Set<Address>> deniedAddresses;

  public LineaTransactionPoolValidatorFactory(
      final BesuConfiguration besuConfiguration,
      final BlockchainService blockchainService,
      final WorldStateService worldStateService,
      final TransactionSimulationService transactionSimulationService,
      final LineaTransactionPoolValidatorConfiguration txPoolValidatorConf,
      final LineaProfitabilityConfiguration profitabilityConf,
      final LineaTracerConfiguration tracerConfiguration,
      final LineaL1L2BridgeSharedConfiguration l1L2BridgeConfiguration,
      final Optional<JsonRpcManager> rejectedTxJsonRpcManager,
      final LineaRlnValidatorConfiguration rlnValidatorConf,
      final SharedServiceManager sharedServiceManager,
      final boolean rlnProverForwarderEnabled,
      final InvalidTransactionByLineCountCache invalidTransactionByLineCountCache,
      final LineaNodeType nodeType) {
    this.besuConfiguration = besuConfiguration;
    this.blockchainService = blockchainService;
    this.worldStateService = worldStateService;
    this.transactionSimulationService = transactionSimulationService;
    this.txPoolValidatorConf = txPoolValidatorConf;
    this.profitabilityConf = profitabilityConf;
    this.tracerConfiguration = tracerConfiguration;
    this.l1L2BridgeConfiguration = l1L2BridgeConfiguration;
    this.rejectedTxJsonRpcManager = rejectedTxJsonRpcManager;
    this.rlnValidatorConf = rlnValidatorConf;
    this.sharedServiceManager = sharedServiceManager;
    this.rlnProverForwarderEnabled = rlnProverForwarderEnabled;
    this.invalidTransactionByLineCountCache = invalidTransactionByLineCountCache;
    this.nodeType = nodeType;

    this.deniedAddresses = new AtomicReference<>(txPoolValidatorConf.deniedAddresses());
  }

  /**
   * Creates a new transaction pool validator, that simply calls in sequence all the actual
   * validators, in a fail-fast mode.
   *
   * @return the new transaction pool validator
   */
  @Override
  public PluginTransactionPoolValidator createTransactionValidator() {
    final var validatorsList = new ArrayList<PluginTransactionPoolValidator>();

    // Conditionally add RLN Prover Forwarder (enabled via configuration flag)
    // Keep it first so we forward local txs before any other validator rejects them.
    if (rlnProverForwarderEnabled) {
      validatorsList.add(
          new RlnProverForwarderValidator(
              rlnValidatorConf,
              true, // enabled
              sharedServiceManager.getKarmaServiceClient(),
              transactionSimulationService,
              blockchainService,
              worldStateService,
              tracerConfiguration,
              l1L2BridgeConfiguration));
    }

    validatorsList.add(new TraceLineLimitValidator(invalidTransactionByLineCountCache));
    validatorsList.add(new AllowedAddressValidator(deniedAddresses));
    validatorsList.add(new GasLimitValidator(txPoolValidatorConf.maxTxGasLimit()));
    validatorsList.add(new CalldataValidator(txPoolValidatorConf.maxTxCalldataSize()));
    validatorsList.add(
        new ProfitabilityValidator(besuConfiguration, blockchainService, profitabilityConf));

    // Conditionally add RLN Validator (for proof verification)
    // Only add RlnVerifierValidator on SEQUENCER nodes, not RPC nodes
    // RPC nodes need RLN enabled for shared services (WorldStateService, etc.) but should NOT
    // reject transactions
    if (rlnValidatorConf.rlnValidationEnabled() && nodeType == LineaNodeType.SEQUENCER) {
      validatorsList.add(
          new RlnVerifierValidator(
              rlnValidatorConf,
              blockchainService,
              sharedServiceManager.getDenyListManager(),
              sharedServiceManager.getKarmaServiceClient(),
              sharedServiceManager.getNullifierTracker()));
    }

    validatorsList.add(
        new SimulationValidator(
            blockchainService,
            worldStateService,
            transactionSimulationService,
            txPoolValidatorConf,
            tracerConfiguration,
            l1L2BridgeConfiguration,
            rejectedTxJsonRpcManager));

    final PluginTransactionPoolValidator[] validators =
        validatorsList.toArray(new PluginTransactionPoolValidator[0]);

    return (transaction, isLocal, hasPriority) -> {
      for (final PluginTransactionPoolValidator validator : validators) {
        final Optional<String> maybeError =
            validator.validateTransaction(transaction, isLocal, hasPriority);
        if (maybeError.isPresent()) {
          if (LOG.isDebugEnabled()) {
            LOG.debug(
                "Tx {} rejected by {} (isLocal={}, hasPriority={}): {}",
                transaction.getHash(),
                validator.getClass().getSimpleName(),
                isLocal,
                hasPriority,
                maybeError.get());
          }
          return maybeError;
        }
      }
      return Optional.empty();
    };
  }

  public void setDeniedAddresses(final Set<Address> deniedAddresses) {
    this.deniedAddresses.set(deniedAddresses);
  }
}
