#include "unilyrics.hpp"
#include <cassert>
#include <future>
#include <iostream>
#include <type_traits>

static unilyrics::adapter wrapped() {
    return {
        [](std::string_view input) { return input.substr(0, 7) == "CUSTOM:"; },
        [](std::string_view input) {
            if (input.substr(0, 7) != "CUSTOM:") throw std::runtime_error("Missing CUSTOM prefix");
            return unilyrics::parse(input.substr(7), UNILYRICS_JSON);
        },
        [](std::string_view document, bool) { return unilyrics::export_result("CUSTOM:" + std::string(document)); }
    };
}
static void contains(const std::string &reply, std::string_view expected) { assert(reply.find(expected) != std::string::npos); }

int main(int argc, char **argv) {
    if (argc > 1 && std::string(argv[1]) == "--protocol") {
        for (std::string input; std::getline(std::cin, input);) std::cout << unilyrics::process(input) << '\n';
        return 0;
    }
    unilyrics::registry registry;
    registry.register_format("x-wrapped", wrapped());
    if (argc > 1 && std::string(argv[1]) == "--custom-protocol") {
        for (std::string document; std::getline(std::cin, document);) {
            auto input = "CUSTOM:" + document;
            std::cout << registry.parse(input) << '\n' << registry.detect(input) << '\n'
                      << registry.export_lyrics(document, "x-wrapped", true) << '\n';
        }
        return 0;
    }
    static_assert(!std::is_copy_constructible<unilyrics::registry>::value, "Native ownership cannot be copied");
    static_assert(std::is_nothrow_move_constructible<unilyrics::registry>::value, "Registry must move safely");
    const std::string document = R"({"version":1,"cues":[{"id":"cue","start_us":0,"segments":[{"text":"සිංහල 👩🏽‍💻"}]}],"attributes":{"precise":18446744073709551615}})";
    contains(registry.export_lyrics(document, "x-wrapped", true), "\"losses\":[]");
    contains(registry.parse("CUSTOM:" + document), "සිංහල 👩🏽‍💻");
    contains(registry.detect("CUSTOM:" + document), "x-wrapped");
    contains(registry.parse("[00:01]hi", "lrc"), "1000000");
    try { registry.register_format("x-wrapped", wrapped()); assert(false); }
    catch (const std::runtime_error &error) { contains(error.what(), "DUPLICATE_FORMAT"); }
    auto mutate = wrapped();
    mutate.export_lyrics = [](std::string_view, bool) { return unilyrics::export_result(R"(CUSTOM:{"version":1,"cues":[]})"); };
    registry.register_format("x-mutate", std::move(mutate));
    contains(registry.export_lyrics(document, "x-mutate"), "CUSTOM_ROUNDTRIP_CHANGED");
    contains(registry.export_lyrics(document, "x-mutate", true), "LOSSY_CONVERSION");
    auto throwing = wrapped();
    throwing.matches = {};
    throwing.parse = [](std::string_view) -> std::string { throw std::runtime_error("parse exception \"quoted\""); };
    registry.register_format("x-throw", std::move(throwing));
    contains(registry.parse("hi", "x-throw"), "CUSTOM_CALLBACK_ERROR");
    contains(registry.export_lyrics(document, "x-throw"), "CUSTOM_ROUNDTRIP_UNVERIFIED");
    contains(registry.export_lyrics(document, "x-throw", true), "LOSSY_CONVERSION");
    auto exporter = wrapped(); exporter.matches = {};
    exporter.export_lyrics = [](std::string_view, bool) -> std::string { throw 42; };
    registry.register_format("x-export-error", std::move(exporter));
    contains(registry.export_lyrics(document, "x-export-error"), "Non-standard C++ callback exception");
    unilyrics::registry detector;
    detector.register_format("x-throw", {[](std::string_view) -> bool { throw std::logic_error("detector exception"); }, {}, {}});
    for (int repeat = 0; repeat < 2; ++repeat) {
        try { detector.detect("hi"); assert(false); }
        catch (const std::logic_error &error) { assert(std::string(error.what()) == "detector exception"); }
    }
    // A nested failure must not leak through the outer call's thread-local error frame.
    unilyrics::registry nested;
    nested.register_format("x-nested", {[&detector](std::string_view) {
        try { detector.detect("hi"); } catch (const std::logic_error &) { }
        return false;
    }, {}, {}});
    contains(nested.detect("hi"), "plain");
    registry.register_format("x-invalid", {{}, [](std::string_view) { return std::string("not json"); }, {}});
    contains(registry.parse("hi", "x-invalid"), "INVALID_CALLBACK_RESULT");
    registry.register_format("x-reentrant", {{}, [&registry](std::string_view input) {
        registry.register_format("x-inside", {});
        return unilyrics::parse(input, UNILYRICS_PLAIN);
    }, {}});
    contains(registry.parse("hi", "x-reentrant"), "\"text\":\"hi\"");
    std::weak_ptr<int> retained;
    {
        unilyrics::registry owner;
        auto token = std::make_shared<int>(7); retained = token;
        owner.register_format("x-owned", {{}, [token](std::string_view input) {
            assert(*token == 7); return unilyrics::parse(input, UNILYRICS_PLAIN);
        }, {}});
        token.reset(); assert(!retained.expired());
        unilyrics::registry moved = std::move(owner);
        contains(moved.parse("hi", "x-owned"), "\"ok\":true");
        try { owner.detect("hi"); assert(false); } catch (const std::logic_error &) { }
        unilyrics::registry assigned;
        assigned = std::move(moved);
        contains(assigned.parse("hi", "x-owned"), "\"ok\":true");
    }
    assert(retained.expired());
    std::weak_ptr<int> rejected_weak;
    {
        auto rejected = std::make_shared<int>(1); rejected_weak = rejected;
        unilyrics::adapter duplicate{{}, [rejected](std::string_view input) { return unilyrics::parse(input); }, {}};
        rejected.reset();
        try { registry.register_format("x-wrapped", std::move(duplicate)); assert(false); } catch (const std::runtime_error &) { }
    }
    assert(rejected_weak.expired());
    unilyrics::registry concurrent;
    concurrent.register_format("x-wrapped", wrapped());
    std::vector<std::future<void>> jobs;
    for (int index = 0; index < 4; ++index) {
        jobs.push_back(std::async(std::launch::async, [&concurrent, document, index] {
            concurrent.register_format("x-extra-" + std::to_string(index), {});
            for (int repeat = 0; repeat < 10; ++repeat) {
                contains(concurrent.parse("CUSTOM:" + document), "\"ok\":true");
                contains(concurrent.export_lyrics(document, "x-wrapped", true), "\"losses\":[]");
            }
        }));
    }
    for (auto &job : jobs) job.get();
    std::cout << "C++ registry ownership, move, callback exception and concurrency checks passed.\n";
}
