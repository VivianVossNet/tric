// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: TRIC+ C++ client — header-only RAII wrapper around the C bridge, exception-free, C++17.

#ifndef TRIC_HPP
#define TRIC_HPP

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

extern "C" {
#include "tric.h"
}

namespace tric {

class connection {
public:
    explicit connection(std::string_view socket_path) noexcept
        : handle_{}, owned_(false)
    {
        std::string path(socket_path);
        handle_ = ::create_connection(path.c_str());
        owned_ = handle_.socket_fd >= 0;
    }

    ~connection() noexcept {
        if (owned_) {
            ::delete_connection(&handle_);
        }
    }

    connection(const connection&) = delete;
    connection& operator=(const connection&) = delete;

    connection(connection&& other) noexcept
        : handle_(other.handle_), owned_(other.owned_)
    {
        other.owned_ = false;
    }

    connection& operator=(connection&& other) noexcept {
        if (this != &other) {
            if (owned_) {
                ::delete_connection(&handle_);
            }
            handle_ = other.handle_;
            owned_ = other.owned_;
            other.owned_ = false;
        }
        return *this;
    }

    bool valid() const noexcept {
        return owned_ && ::check_connection(&handle_) != 0;
    }

    explicit operator bool() const noexcept {
        return valid();
    }

    std::optional<std::string> read(std::string_view key) noexcept {
        TricValue v = ::read_value(
            &handle_,
            reinterpret_cast<const std::uint8_t*>(key.data()),
            key.size()
        );
        if (v.data == nullptr) {
            return std::nullopt;
        }
        std::string out(reinterpret_cast<const char*>(v.data), v.length);
        ::delete_value_result(&v);
        return out;
    }

    bool write(std::string_view key, std::string_view value) noexcept {
        return ::write_value(
            &handle_,
            reinterpret_cast<const std::uint8_t*>(key.data()), key.size(),
            reinterpret_cast<const std::uint8_t*>(value.data()), value.size()
        ) == 0;
    }

    bool del(std::string_view key) noexcept {
        return ::delete_value(
            &handle_,
            reinterpret_cast<const std::uint8_t*>(key.data()), key.size()
        ) == 0;
    }

    bool cad(std::string_view key, std::string_view expected) noexcept {
        return ::delete_value_if_match(
            &handle_,
            reinterpret_cast<const std::uint8_t*>(key.data()), key.size(),
            reinterpret_cast<const std::uint8_t*>(expected.data()), expected.size()
        ) == 1;
    }

    bool ttl(std::string_view key, std::uint64_t duration_ms) noexcept {
        return ::write_ttl(
            &handle_,
            reinterpret_cast<const std::uint8_t*>(key.data()), key.size(),
            duration_ms
        ) == 0;
    }

    std::vector<std::pair<std::string, std::string>> scan(std::string_view prefix) {
        TricScanResult r = ::find_by_prefix(
            &handle_,
            reinterpret_cast<const std::uint8_t*>(prefix.data()),
            prefix.size()
        );
        std::vector<std::pair<std::string, std::string>> out;
        out.reserve(r.count);
        for (std::size_t i = 0; i < r.count; ++i) {
            out.emplace_back(
                std::string(reinterpret_cast<const char*>(r.pairs[i].key),   r.pairs[i].key_length),
                std::string(reinterpret_cast<const char*>(r.pairs[i].value), r.pairs[i].value_length)
            );
        }
        ::delete_scan_result(&r);
        return out;
    }

private:
    TricConnection handle_;
    bool           owned_;
};

} // namespace tric

#endif
