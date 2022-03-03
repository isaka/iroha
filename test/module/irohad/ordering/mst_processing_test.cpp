/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include <gtest/gtest.h>
#include <gmock/gmock.h>
#include <memory>

#include "framework/crypto_literals.hpp"
#include "module/irohad/ordering/mst_test_helpers.hpp"
#include "ordering/impl/batches_cache.hpp"

using ::testing::ByMove;
using ::testing::Ref;
using ::testing::Return;
using ::testing::ReturnRefOfCopy;

struct MSTProcessingTest : public ::testing::Test {
  void SetUp() override {
    batches_cache_ = std::make_shared<iroha::ordering::BatchesCache>();
  }

  std::shared_ptr<iroha::ordering::BatchesCache> batches_cache_;
};

TEST_F(MSTProcessingTest, SimpleAdd) {
  auto batch = addSignaturesFromKeyPairs(
      makeTestBatch(txBuilder(1, iroha::time::now(), 1)), 0, makeKey());
  batches_cache_->insert(batch);
  ASSERT_EQ(batches_cache_->availableTxsCount(), 1);
}

TEST_F(MSTProcessingTest, SimpleUnsubscribedAdd) {
  auto batch = addSignaturesFromKeyPairs(
      makeTestBatch(txBuilder(1, iroha::time::now(), 2)), 0, makeKey());
  batches_cache_->insert(batch);
  ASSERT_EQ(batches_cache_->availableTxsCount(), 0);
}

TEST_F(MSTProcessingTest, SubscribedAdd) {
  auto first_signature = makeSignature("1"_hex_sig, "pub_key_1"_hex_pubkey);
  auto second_signature = makeSignature("2"_hex_sig, "pub_key_2"_hex_pubkey);
  auto base_tx = makeTestBatch(txBuilder(1, iroha::time::now(), 2));

  auto first_tx = addSignatures(base_tx, 0, first_signature);
  batches_cache_->insert(first_tx);
  ASSERT_EQ(batches_cache_->availableTxsCount(), 0);

  auto second_tx = addSignatures(base_tx, 0, second_signature);
  batches_cache_->insert(second_tx);
  ASSERT_EQ(batches_cache_->availableTxsCount(), 1);
}