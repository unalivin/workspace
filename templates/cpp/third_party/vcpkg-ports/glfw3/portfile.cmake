vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO glfw/glfw
    REF "${VERSION}"
    SHA512 42e8b8bdc2ecaab8d33d43cab6cf6e4bfd1f23492a5f13ec4b3762b74661dc3c9625721f436fbf841909b9af3ac6dcb8080201b36592d33c2bb196f768fc307f
    HEAD_REF master
)

set(platform_options "")
if(VCPKG_TARGET_IS_LINUX)
    message(STATUS "GLFW is built against the X11 headers of the system: libxinerama-dev libxcursor-dev xorg-dev libglu1-mesa-dev pkg-config on Debian and Ubuntu")
    list(APPEND platform_options -DGLFW_BUILD_X11=ON -DGLFW_BUILD_WAYLAND=OFF)
endif()

vcpkg_cmake_configure(
    SOURCE_PATH "${SOURCE_PATH}"
    OPTIONS
        -DGLFW_BUILD_EXAMPLES=OFF
        -DGLFW_BUILD_TESTS=OFF
        -DGLFW_BUILD_DOCS=OFF
        ${platform_options}
)

vcpkg_cmake_install()
vcpkg_cmake_config_fixup(CONFIG_PATH lib/cmake/glfw3)
vcpkg_fixup_pkgconfig()
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")
vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE.md")
