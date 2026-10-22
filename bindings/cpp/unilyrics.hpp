#ifndef UNILYRICS_HPP
#define UNILYRICS_HPP
#include "unilyrics.h"
#include <memory>
#include <exception>
#include <functional>
#include <mutex>
#include <utility>
#include <vector>
#include <stdexcept>
#include <string>
#include <string_view>

namespace unilyrics {
inline std::string take_response(char *value) {
    std::unique_ptr<char, decltype(&unilyrics_free)> response(
        value, &unilyrics_free);
    if (!response) throw std::runtime_error("UniLyrics returned no response");
    return response.get();
}
// Responses are JSON; consumers may use their existing JSON library.
inline std::string process(std::string_view request) {
    return take_response(unilyrics_process(reinterpret_cast<const uint8_t *>(request.data()), request.size()));
}
inline std::string parse(std::string_view text, unilyrics_format format = UNILYRICS_AUTO) {
    return take_response(unilyrics_parse(reinterpret_cast<const uint8_t *>(text.data()), text.size(), format));
}
inline std::string detect(std::string_view text) {
    return take_response(unilyrics_detect(reinterpret_cast<const uint8_t *>(text.data()), text.size()));
}
inline std::string export_lyrics(std::string_view document_json, unilyrics_format format, bool strict = false) {
    return take_response(unilyrics_export(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size(), format, strict));
}
inline std::string inspect(std::string_view document_json) {
    return take_response(unilyrics_inspect(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size()));
}
inline std::string normalize(std::string_view document_json, int64_t offset_us = 0) {
    return take_response(unilyrics_normalize(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size(), offset_us));
}
inline std::string project(std::string_view document_json, std::string_view options_json = "{}") {
    return take_response(unilyrics_project(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size(), reinterpret_cast<const uint8_t *>(options_json.data()), options_json.size()));
}
inline std::string export_profile(std::string_view document_json, unilyrics_format format, std::string_view options_json = "{}", bool strict = false) {
    return take_response(unilyrics_export_profile(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size(), format, strict, reinterpret_cast<const uint8_t *>(options_json.data()), options_json.size()));
}
inline std::string export_preserved(std::string_view document_json, unilyrics_format format, std::string_view options_json = "{}") {
    return take_response(unilyrics_export_preserved(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size(), format, reinterpret_cast<const uint8_t *>(options_json.data()), options_json.size()));
}
inline std::string restore_preserved(std::string_view output, unilyrics_format format, std::string_view sidecar_json) {
    return take_response(unilyrics_restore_preserved(reinterpret_cast<const uint8_t *>(output.data()), output.size(), format, reinterpret_cast<const uint8_t *>(sidecar_json.data()), sidecar_json.size()));
}
inline std::string validate(std::string_view document_json) {
    return take_response(unilyrics_validate(reinterpret_cast<const uint8_t *>(document_json.data()), document_json.size()));
}
inline std::string export_result(std::string_view output) {
    return take_response(unilyrics_export_result(reinterpret_cast<const uint8_t *>(output.data()), output.size()));
}
inline std::string error_result(std::string_view code, std::string_view message) {
    return take_response(unilyrics_error_result(reinterpret_cast<const uint8_t *>(code.data()), code.size(),
        reinterpret_cast<const uint8_t *>(message.data()), message.size()));
}

struct adapter {
    std::function<bool(std::string_view)> matches;
    // Parse/export callbacks return the usual JSON reply envelope, not just its result.
    std::function<std::string(std::string_view)> parse;
    std::function<std::string(std::string_view, bool)> export_lyrics;
};

class registry {
    struct state {
        std::mutex mutex;
        std::vector<std::unique_ptr<adapter>> callbacks;
        // Destroy the native handle before the borrowed callback contexts.
        std::unique_ptr<unilyrics_registry, decltype(&unilyrics_registry_free)> handle{
            unilyrics_registry_new(), &unilyrics_registry_free};
        state() { if (!handle) throw std::bad_alloc(); }
    };
    std::unique_ptr<state> state_ = std::make_unique<state>();
    inline static thread_local std::exception_ptr detector_error_;

    struct call_frame {
        std::exception_ptr previous = std::exchange(detector_error_, nullptr);
        ~call_frame() { detector_error_ = previous; }
        void check() const { if (detector_error_) std::rethrow_exception(detector_error_); }
    };

    state &get_state() const {
        if (!state_) throw std::logic_error("Registry has been moved from");
        return *state_;
    }
    static char *callback_error(std::string_view message) noexcept {
        constexpr std::string_view code = "CUSTOM_CALLBACK_ERROR";
        return unilyrics_error_result(reinterpret_cast<const uint8_t *>(code.data()), code.size(),
            reinterpret_cast<const uint8_t *>(message.data()), message.size());
    }
    static int32_t match_callback(void *context, const uint8_t *data, size_t length) noexcept {
        if (detector_error_) return 0;
        try {
            return static_cast<adapter *>(context)->matches(std::string_view(reinterpret_cast<const char *>(data), length)) ? 1 : 0;
        } catch (...) {
            detector_error_ = std::current_exception();
            return 0;
        }
    }
    static char *parse_callback(void *context, const uint8_t *data, size_t length) noexcept {
        if (detector_error_) return callback_error("Custom detector failed");
        try {
            auto reply = static_cast<adapter *>(context)->parse(std::string_view(reinterpret_cast<const char *>(data), length));
            return unilyrics_copy_reply(reinterpret_cast<const uint8_t *>(reply.data()), reply.size());
        } catch (const std::exception &error) { return callback_error(error.what()); }
        catch (...) { return callback_error("Non-standard C++ callback exception"); }
    }
    static char *export_callback(void *context, const uint8_t *data, size_t length, int32_t strict) noexcept {
        try {
            auto reply = static_cast<adapter *>(context)->export_lyrics(std::string_view(reinterpret_cast<const char *>(data), length), strict != 0);
            return unilyrics_copy_reply(reinterpret_cast<const uint8_t *>(reply.data()), reply.size());
        } catch (const std::exception &error) { return callback_error(error.what()); }
        catch (...) { return callback_error("Non-standard C++ callback exception"); }
    }
    std::string call(std::string_view input, int operation, std::string_view format, bool strict) const {
        auto &state = get_state();
        call_frame frame;
        auto reply = take_response(unilyrics_registry_call(state.handle.get(), reinterpret_cast<const uint8_t *>(input.data()), input.size(),
            operation, reinterpret_cast<const uint8_t *>(format.data()), format.size(), strict ? 1 : 0));
        frame.check();
        return reply;
    }

public:
    registry() = default;
    registry(const registry &) = delete;
    registry &operator=(const registry &) = delete;
    registry(registry &&) noexcept = default;
    registry &operator=(registry &&) noexcept = default;

    void register_format(std::string_view name, adapter callbacks) {
        auto &state = get_state();
        auto context = std::make_unique<adapter>(std::move(callbacks));
        std::unique_lock<std::mutex> lock(state.mutex);
        // Own the context before registration: allocation failure must not leave a dangling native pointer.
        state.callbacks.push_back(std::move(context));
        auto *borrowed = state.callbacks.back().get();
        char *error = nullptr;
        auto ok = unilyrics_registry_register(state.handle.get(), reinterpret_cast<const uint8_t *>(name.data()), name.size(), borrowed,
            borrowed->matches ? &match_callback : nullptr, borrowed->parse ? &parse_callback : nullptr,
            borrowed->export_lyrics ? &export_callback : nullptr, &error);
        std::unique_ptr<char, decltype(&unilyrics_free)> reply(error, &unilyrics_free);
        if (!ok) { context = std::move(state.callbacks.back()); state.callbacks.pop_back(); }
        lock.unlock();
        if (!ok) throw std::runtime_error(reply ? reply.get() : "Native registration failed");
    }
    std::string parse(std::string_view input, std::string_view format = {}) const { return call(input, 0, format, false); }
    std::string detect(std::string_view input) const { return call(input, 1, {}, false); }
    std::string export_lyrics(std::string_view document_json, std::string_view format, bool strict = false) const {
        return call(document_json, 2, format, strict);
    }
};

}
#endif
