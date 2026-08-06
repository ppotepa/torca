#pragma once

#include <flutter/binary_messenger.h>
#include <flutter/encodable_value.h>
#include <flutter/method_channel.h>

#include <memory>

namespace torca {

// The concrete implementation must delegate to the Rust EngineBridge and must
// not duplicate any pairing, messaging or persistence workflow in C++.
class NativeEngine {
 public:
  virtual ~NativeEngine() = default;
  virtual flutter::EncodableMap Execute(
      const flutter::EncodableMap& command) = 0;
  virtual flutter::EncodableMap Snapshot() = 0;
  virtual void Close() = 0;
};

class TorcaEngineChannel {
 public:
  TorcaEngineChannel(flutter::BinaryMessenger* messenger,
                     std::unique_ptr<NativeEngine> engine);
  ~TorcaEngineChannel();

  TorcaEngineChannel(const TorcaEngineChannel&) = delete;
  TorcaEngineChannel& operator=(const TorcaEngineChannel&) = delete;

  void PublishSnapshot();
  void Close();

 private:
  bool HasSupportedVersion(const flutter::EncodableMap& arguments) const;
  flutter::EncodableMap SnapshotWithVersion() const;

  std::unique_ptr<NativeEngine> engine_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
  bool closed_ = false;
};

}  // namespace torca
