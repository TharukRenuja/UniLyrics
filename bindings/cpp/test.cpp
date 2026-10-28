#include "unilyrics.hpp"
#include <cassert>
#include <iostream>
int main(int argc, char **argv) {
    if (argc > 1 && std::string(argv[1]) == "--protocol") {
        for (std::string input; std::getline(std::cin, input);) {
            std::cout << unilyrics::process(input) << '\n';
        }
        return 0;
    }
    auto response = unilyrics::process(R"({"op":"parse","input":"[00:01.25]你好","format":"lrc"})");
    assert(response.find("1250000") != std::string::npos);
    assert(response.find("你好") != std::string::npos);
    assert(unilyrics::parse("[00:01]hi").find("1000000") != std::string::npos);
    assert(unilyrics::detect("[00:01]hi").find("lrc") != std::string::npos);
    const auto document = R"({"version":1,"cues":[],"metadata":[],"attributes":{}})";
    assert(unilyrics::inspect(document).find("\"agents\":false") != std::string::npos);
    assert(unilyrics::validate(document).find("\"ok\":true") != std::string::npos);
    assert(unilyrics::export_lyrics(document, UNILYRICS_PLAIN).find("\"output\":\"\"") != std::string::npos);
    assert(unilyrics::normalize(R"({"version":1,"cues":[{"id":"cue","start_us":100,"segments":[]}]})", -100).find("TIMING_SHIFTED") != std::string::npos);
    const auto timed = R"({"version":1,"cues":[{"id":"cue","start_us":100,"segments":[]}]})";
    assert(unilyrics::project(timed).find("\"start_us\":100") != std::string::npos);
    const auto options = R"({"profile":"custom","features":[]})";
    assert(unilyrics::project(timed, options).find("PROFILE_LINE_TIMING_DROPPED") != std::string::npos);
    assert(unilyrics::export_profile(timed, UNILYRICS_JSON, options, true).find("LOSSY_CONVERSION") != std::string::npos);
    assert(unilyrics::export_preserved(document, UNILYRICS_PLAIN).find("sidecar") != std::string::npos);
    const auto sidecar = R"({"version":1,"format":"plain","output":"","document":{"version":1,"cues":[]}})";
    assert(unilyrics::restore_preserved("", UNILYRICS_PLAIN, sidecar).find("SIDECAR_RESTORED") != std::string::npos);
    assert(unilyrics::restore_preserved("changed", UNILYRICS_PLAIN, sidecar).find("SIDECAR_MISMATCH") != std::string::npos);
    return 0;
}
