#include "torca_engine_channel.h"

#include <flutter/method_call.h>
#include <flutter/method_result_functions.h>
#include <flutter/standard_method_codec.h>

#include <string>
#include <utility>

namespace torca {
namespace {
constexpr char kChannelName[] = "torca.engine.v1";
constexpr int32_t kContractVersion = 1;
const flutter::EncodableValue kVersionKey("contractVersion");
const flutter::EncodableValue kCommandKey("command");
}  // namespace

TorcaEngineChannel::TorcaEngineChannel(
    flutter::BinaryMessenger* messenger,
    std::unique_ptr<NativeEngine> engine)
    : engine_(std::move(engine)),
      channel_(std::make_unique<
               flutter::MethodChannel<flutter::EncodableValue>>(
          messenger, kChannelName,
          &flutter::StandardMethodCodec::GetInstance())) {
  channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<
                 flutter::MethodResult<flutter::EncodableValue>> result) {
        if (closed_) {
          result->Error("engine_closed", "Torca engine is closed");
          return;
        }
        const auto* arguments =
            std::get_if<flutter::EncodableMap>(call.arguments());
        if (arguments == nullptr || !HasSupportedVersion(*arguments)) {
          result->Error("invalid_engine_request",
                        "Unsupported Torca contract version");
          return;
        }

        try {
          if (call.method_name() == "snapshot") {
            result->Success(flutter::EncodableValue(SnapshotWithVersion()));
            return;
          }
          if (call.method_name() == "execute") {
            const auto command_iterator = arguments->find(kCommandKey);
            if (command_iterator == arguments->end()) {
              result->Error("invalid_engine_request", "Missing command map");
              return;
            }
            const auto* command =
                std::get_if<flutter::EncodableMap>(&command_iterator->second);
            if (command == nullptr) {
              result->Error("invalid_engine_request",
                            "Command must be a map");
              return;
            }
            auto response = engine_->Execute(*command);
            result->Success(flutter::EncodableValue(std::move(response)));
            PublishSnapshot();
            return;
          }
          result->NotImplemented();
        } catch (...) {
          result->Error("native_engine_failure",
                        "Native Torca engine operation failed");
        }
      });
}

TorcaEngineChannel::~TorcaEngineChannel() { Close(); }

void TorcaEngineChannel::PublishSnapshot() {
  if (closed_) {
    return;
  }
  channel_->InvokeMethod(
      "snapshotChanged",
      std::make_unique<flutter::EncodableValue>(SnapshotWithVersion()));
}

void TorcaEngineChannel::Close() {
  if (closed_) {
    return;
  }
  closed_ = true;
  channel_->SetMethodCallHandler(nullptr);
  if (engine_ != nullptr) {
    engine_->Close();
  }
}

bool TorcaEngineChannel::HasSupportedVersion(
    const flutter::EncodableMap& arguments) const {
  const auto iterator = arguments.find(kVersionKey);
  if (iterator == arguments.end()) {
    return false;
  }
  const auto* version = std::get_if<int32_t>(&iterator->second);
  return version != nullptr && *version == kContractVersion;
}

flutter::EncodableMap TorcaEngineChannel::SnapshotWithVersion() const {
  auto snapshot = engine_->Snapshot();
  snapshot[kVersionKey] = flutter::EncodableValue(kContractVersion);
  return snapshot;
}

}  // namespace torca
