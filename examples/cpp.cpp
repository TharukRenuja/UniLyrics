#include "unilyrics.hpp"
#include <iostream>

int main() {
    try {
        std::cout << unilyrics::parse("[00:01]Hello 世界 🎵", UNILYRICS_LRC) << '\n';
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
