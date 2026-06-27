cmake_minimum_required(VERSION 3.18)
project(varmlennative C)

# Prebuilt tun2socks (staged into jniLibs by the build setup).
add_library(hev-socks5-tunnel SHARED IMPORTED)
set_target_properties(hev-socks5-tunnel PROPERTIES IMPORTED_LOCATION
    ${CMAKE_CURRENT_SOURCE_DIR}/../jniLibs/${ANDROID_ABI}/libhev-socks5-tunnel.so)

add_library(tproxy SHARED tproxy.c)
target_include_directories(tproxy PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/hev-include)
target_link_libraries(tproxy hev-socks5-tunnel log)
