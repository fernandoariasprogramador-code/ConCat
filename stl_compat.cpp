// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
#include <cstdint>
#include <cstddef>
#include <cstdlib>
#include <cstring>

struct _Min_max_element_t {
    const void* _Min;
    const void* _Max;
};

extern "C" {

// 1. Abort
[[noreturn]] void __cdecl __std_init_once_link_alternate_names_and_abort() noexcept {
    abort();
}

// 2. Count
size_t __stdcall __std_count_trivial_8(const void* const _First, const void* const _Last, const uint64_t _Val) noexcept {
    auto p = (const uint64_t*)_First;
    auto end = (const uint64_t*)_Last;
    size_t c = 0;
    for (; p < end; ++p) if (*p == _Val) ++c;
    return c;
}

// 3. Mismatch
size_t __stdcall __std_mismatch_1(const void* const _First1, const void* const _First2, const size_t _Count) noexcept {
    auto p1 = (const uint8_t*)_First1;
    auto p2 = (const uint8_t*)_First2;
    for (size_t i = 0; i < _Count; ++i) if (p1[i] != p2[i]) return i;
    return _Count;
}

size_t __stdcall __std_mismatch_8(const void* const _First1, const void* const _First2, const size_t _Count) noexcept {
    auto p1 = (const uint64_t*)_First1;
    auto p2 = (const uint64_t*)_First2;
    for (size_t i = 0; i < _Count; ++i) if (p1[i] != p2[i]) return i;
    return _Count;
}

// 4. Find trivial
const void* __stdcall __std_find_trivial_1(const void* const _First, const void* const _Last, const uint8_t _Val) noexcept {
    auto p = (const uint8_t*)_First;
    auto end = (const uint8_t*)_Last;
    for (; p < end; ++p) if (*p == _Val) return p;
    return _Last;
}

const void* __stdcall __std_find_trivial_2(const void* const _First, const void* const _Last, const uint16_t _Val) noexcept {
    auto p = (const uint16_t*)_First;
    auto end = (const uint16_t*)_Last;
    for (; p < end; ++p) if (*p == _Val) return p;
    return _Last;
}

const void* __stdcall __std_find_trivial_4(const void* const _First, const void* const _Last, const uint32_t _Val) noexcept {
    auto p = (const uint32_t*)_First;
    auto end = (const uint32_t*)_Last;
    for (; p < end; ++p) if (*p == _Val) return p;
    return _Last;
}

const void* __stdcall __std_find_trivial_8(const void* const _First, const void* const _Last, const uint64_t _Val) noexcept {
    auto p = (const uint64_t*)_First;
    auto end = (const uint64_t*)_Last;
    for (; p < end; ++p) if (*p == _Val) return p;
    return _Last;
}

// 5. Find last trivial
const void* __stdcall __std_find_last_trivial_1(const void* const _First, const void* const _Last, const uint8_t _Val) noexcept {
    auto p = (const uint8_t*)_Last;
    auto begin = (const uint8_t*)_First;
    while (p > begin) {
        --p;
        if (*p == _Val) return p;
    }
    return _Last;
}

const void* __stdcall __std_find_last_trivial_2(const void* const _First, const void* const _Last, const uint16_t _Val) noexcept {
    auto p = (const uint16_t*)_Last;
    auto begin = (const uint16_t*)_First;
    while (p > begin) {
        --p;
        if (*p == _Val) return p;
    }
    return _Last;
}

// 6. Min element
const void* __stdcall __std_min_element_4(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    if (_Signed) {
        auto p = (const int32_t*)_First;
        auto end = (const int32_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p < *best) best = p;
        return best;
    } else {
        auto p = (const uint32_t*)_First;
        auto end = (const uint32_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p < *best) best = p;
        return best;
    }
}

const void* __stdcall __std_min_element_f(const void* const _First, const void* const _Last, bool) noexcept {
    auto p = (const float*)_First;
    auto end = (const float*)_Last;
    if (p == end) return _First;
    auto best = p++;
    for (; p < end; ++p) if (*p < *best) best = p;
    return best;
}

const void* __stdcall __std_min_element_d(const void* const _First, const void* const _Last, bool) noexcept {
    auto p = (const double*)_First;
    auto end = (const double*)_Last;
    if (p == end) return _First;
    auto best = p++;
    for (; p < end; ++p) if (*p < *best) best = p;
    return best;
}

// 7. Max element
const void* __stdcall __std_max_element_1(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    if (_Signed) {
        auto p = (const int8_t*)_First;
        auto end = (const int8_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    } else {
        auto p = (const uint8_t*)_First;
        auto end = (const uint8_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    }
}

const void* __stdcall __std_max_element_2(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    if (_Signed) {
        auto p = (const int16_t*)_First;
        auto end = (const int16_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    } else {
        auto p = (const uint16_t*)_First;
        auto end = (const uint16_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    }
}

const void* __stdcall __std_max_element_4(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    if (_Signed) {
        auto p = (const int32_t*)_First;
        auto end = (const int32_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    } else {
        auto p = (const uint32_t*)_First;
        auto end = (const uint32_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    }
}

const void* __stdcall __std_max_element_8(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    if (_Signed) {
        auto p = (const int64_t*)_First;
        auto end = (const int64_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    } else {
        auto p = (const uint64_t*)_First;
        auto end = (const uint64_t*)_Last;
        if (p == end) return _First;
        auto best = p++;
        for (; p < end; ++p) if (*p > *best) best = p;
        return best;
    }
}

const void* __stdcall __std_max_element_f(const void* const _First, const void* const _Last, bool) noexcept {
    auto p = (const float*)_First;
    auto end = (const float*)_Last;
    if (p == end) return _First;
    auto best = p++;
    for (; p < end; ++p) if (*p > *best) best = p;
    return best;
}

const void* __stdcall __std_max_element_d(const void* const _First, const void* const _Last, bool) noexcept {
    auto p = (const double*)_First;
    auto end = (const double*)_Last;
    if (p == end) return _First;
    auto best = p++;
    for (; p < end; ++p) if (*p > *best) best = p;
    return best;
}

// 8. Minmax element
_Min_max_element_t __stdcall __std_minmax_element_4(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    _Min_max_element_t res = {_First, _First};
    if (_First == _Last) return res;
    if (_Signed) {
        auto p = (const int32_t*)_First;
        auto end = (const int32_t*)_Last;
        auto min_p = p;
        auto max_p = p++;
        for (; p < end; ++p) {
            if (*p < *min_p) min_p = p;
            if (*p >= *max_p) max_p = p;
        }
        res._Min = min_p; res._Max = max_p;
    } else {
        auto p = (const uint32_t*)_First;
        auto end = (const uint32_t*)_Last;
        auto min_p = p;
        auto max_p = p++;
        for (; p < end; ++p) {
            if (*p < *min_p) min_p = p;
            if (*p >= *max_p) max_p = p;
        }
        res._Min = min_p; res._Max = max_p;
    }
    return res;
}

_Min_max_element_t __stdcall __std_minmax_element_8(const void* const _First, const void* const _Last, const bool _Signed) noexcept {
    _Min_max_element_t res = {_First, _First};
    if (_First == _Last) return res;
    if (_Signed) {
        auto p = (const int64_t*)_First;
        auto end = (const int64_t*)_Last;
        auto min_p = p;
        auto max_p = p++;
        for (; p < end; ++p) {
            if (*p < *min_p) min_p = p;
            if (*p >= *max_p) max_p = p;
        }
        res._Min = min_p; res._Max = max_p;
    } else {
        auto p = (const uint64_t*)_First;
        auto end = (const uint64_t*)_Last;
        auto min_p = p;
        auto max_p = p++;
        for (; p < end; ++p) {
            if (*p < *min_p) min_p = p;
            if (*p >= *max_p) max_p = p;
        }
        res._Min = min_p; res._Max = max_p;
    }
    return res;
}

_Min_max_element_t __stdcall __std_minmax_element_f(const void* const _First, const void* const _Last, bool) noexcept {
    _Min_max_element_t res = {_First, _First};
    if (_First == _Last) return res;
    auto p = (const float*)_First;
    auto end = (const float*)_Last;
    auto min_p = p;
    auto max_p = p++;
    for (; p < end; ++p) {
        if (*p < *min_p) min_p = p;
        if (*p >= *max_p) max_p = p;
    }
    res._Min = min_p; res._Max = max_p;
    return res;
}

// 9. Max scalar
int32_t __stdcall __std_max_4i(const void* const _First, const void* const _Last) noexcept {
    auto p = (const int32_t*)_First;
    auto end = (const int32_t*)_Last;
    int32_t best = *p++;
    for (; p < end; ++p) if (*p > best) best = *p;
    return best;
}

uint64_t __stdcall __std_max_8u(const void* const _First, const void* const _Last) noexcept {
    auto p = (const uint64_t*)_First;
    auto end = (const uint64_t*)_Last;
    uint64_t best = *p++;
    for (; p < end; ++p) if (*p > best) best = *p;
    return best;
}

// 10. Remove
void* __stdcall __std_remove_1(void* _First, void* const _Last, const uint8_t _Val) noexcept {
    auto in = (uint8_t*)_First;
    auto end = (uint8_t*)_Last;
    auto out = in;
    for (; in < end; ++in) if (*in != _Val) *out++ = *in;
    return out;
}

void* __stdcall __std_remove_2(void* _First, void* const _Last, const uint16_t _Val) noexcept {
    auto in = (uint16_t*)_First;
    auto end = (uint16_t*)_Last;
    auto out = in;
    for (; in < end; ++in) if (*in != _Val) *out++ = *in;
    return out;
}

void* __stdcall __std_remove_8(void* _First, void* const _Last, const uint64_t _Val) noexcept {
    auto in = (uint64_t*)_First;
    auto end = (uint64_t*)_Last;
    auto out = in;
    for (; in < end; ++in) if (*in != _Val) *out++ = *in;
    return out;
}

// 11. Search
const void* __stdcall __std_search_1(const void* _First1, const void* _Last1, const void* _First2, size_t _Count2) noexcept {
    if (_Count2 == 0) return _First1;
    auto h = (const uint8_t*)_First1;
    auto h_end = (const uint8_t*)_Last1;
    auto n = (const uint8_t*)_First2;
    if ((size_t)(h_end - h) < _Count2) return _Last1;
    auto limit = h_end - _Count2;
    for (; h <= limit; ++h) {
        if (memcmp(h, n, _Count2) == 0) return h;
    }
    return _Last1;
}

const void* __stdcall __std_search_2(const void* _First1, const void* _Last1, const void* _First2, size_t _Count2) noexcept {
    if (_Count2 == 0) return _First1;
    auto h = (const uint16_t*)_First1;
    auto h_end = (const uint16_t*)_Last1;
    auto n = (const uint16_t*)_First2;
    if ((size_t)(h_end - h) < _Count2) return _Last1;
    auto limit = h_end - _Count2;
    for (; h <= limit; ++h) {
        if (memcmp(h, n, _Count2 * 2) == 0) return h;
    }
    return _Last1;
}

// 12. Find End
const void* __stdcall __std_find_end_1(const void* _First1, const void* _Last1, const void* _First2, size_t _Count2) noexcept {
    if (_Count2 == 0) return _Last1;
    auto h = (const uint8_t*)_First1;
    auto h_end = (const uint8_t*)_Last1;
    auto n = (const uint8_t*)_First2;
    if ((size_t)(h_end - h) < _Count2) return _Last1;
    auto p = h_end - _Count2;
    for (;; --p) {
        if (memcmp(p, n, _Count2) == 0) return p;
        if (p == h) break;
    }
    return _Last1;
}

const void* __stdcall __std_find_end_2(const void* _First1, const void* _Last1, const void* _First2, size_t _Count2) noexcept {
    if (_Count2 == 0) return _Last1;
    auto h = (const uint16_t*)_First1;
    auto h_end = (const uint16_t*)_Last1;
    auto n = (const uint16_t*)_First2;
    if ((size_t)(h_end - h) < _Count2) return _Last1;
    auto p = h_end - _Count2;
    for (;; --p) {
        if (memcmp(p, n, _Count2 * 2) == 0) return p;
        if (p == h) break;
    }
    return _Last1;
}

// 13. Find First Of Trivial Pos
size_t __stdcall __std_find_first_of_trivial_pos_1(const void* const _Haystack, const size_t _Haystack_length, const void* const _Needle, const size_t _Needle_length) noexcept {
    auto h = (const uint8_t*)_Haystack;
    auto n = (const uint8_t*)_Needle;
    for (size_t i = 0; i < _Haystack_length; ++i) {
        for (size_t j = 0; j < _Needle_length; ++j) {
            if (h[i] == n[j]) return i;
        }
    }
    return (size_t)-1;
}

size_t __stdcall __std_find_first_of_trivial_pos_2(const void* const _Haystack, const size_t _Haystack_length, const void* const _Needle, const size_t _Needle_length) noexcept {
    auto h = (const uint16_t*)_Haystack;
    auto n = (const uint16_t*)_Needle;
    for (size_t i = 0; i < _Haystack_length; ++i) {
        for (size_t j = 0; j < _Needle_length; ++j) {
            if (h[i] == n[j]) return i;
        }
    }
    return (size_t)-1;
}

// 14. Find Last Of Trivial Pos
size_t __stdcall __std_find_last_of_trivial_pos_1(const void* const _Haystack, const size_t _Haystack_length, const void* const _Needle, const size_t _Needle_length) noexcept {
    if (_Haystack_length == 0) return (size_t)-1;
    auto h = (const uint8_t*)_Haystack;
    auto n = (const uint8_t*)_Needle;
    for (size_t i = _Haystack_length; i > 0; --i) {
        for (size_t j = 0; j < _Needle_length; ++j) {
            if (h[i - 1] == n[j]) return i - 1;
        }
    }
    return (size_t)-1;
}

size_t __stdcall __std_find_last_of_trivial_pos_2(const void* const _Haystack, const size_t _Haystack_length, const void* const _Needle, const size_t _Needle_length) noexcept {
    if (_Haystack_length == 0) return (size_t)-1;
    auto h = (const uint16_t*)_Haystack;
    auto n = (const uint16_t*)_Needle;
    for (size_t i = _Haystack_length; i > 0; --i) {
        for (size_t j = 0; j < _Needle_length; ++j) {
            if (h[i - 1] == n[j]) return i - 1;
        }
    }
    return (size_t)-1;
}

} // extern "C"
