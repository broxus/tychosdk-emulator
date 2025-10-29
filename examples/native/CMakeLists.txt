cmake_minimum_required(VERSION 3.20)
project(examples LANGUAGES C)

set(CMAKE_C_STANDARD 99)

if(APPLE)
    add_compile_options(-x objective-c)
endif()

cmake_path(GET CMAKE_SOURCE_DIR PARENT_PATH EXAMPLES_ROOT_DIR)
cmake_path(GET EXAMPLES_ROOT_DIR PARENT_PATH PROJECT_ROOT_DIR)

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(TYCHO_EMULATOR_LIB_TYPE debug)
else()
    set(TYCHO_EMULATOR_LIB_TYPE release)
endif()

if(DEFINED ENV{CARGO_BUILD_TARGET})
    set(TYCHO_EMULATOR_LIB_DIR ${PROJECT_ROOT_DIR}/target/$ENV{CARGO_BUILD_TARGET}/${TYCHO_EMULATOR_LIB_TYPE})
else()
    set(TYCHO_EMULATOR_LIB_DIR ${PROJECT_ROOT_DIR}/target/${TYCHO_EMULATOR_LIB_TYPE})
endif()

message("${TYCHO_EMULATOR_LIB_DIR}")

find_library(
    EMULATOR_LIBRARY NAMES libtycho_emulator.so tycho_emulator.dll tycho_emulator
    HINTS ${TYCHO_EMULATOR_LIB_DIR}
    REQUIRED
)

# Example

add_executable(example main.c)

if (MSVC)
    add_compile_options(/W4)
else()
    add_compile_options(-Wall -Wextra -Wpedantic)
endif()

include_directories(${PROJECT_ROOT_DIR}/ffi)

target_link_libraries(example ${EMULATOR_LIBRARY})
