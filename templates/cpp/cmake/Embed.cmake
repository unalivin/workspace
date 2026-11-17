set(PROJECT_EMBED_SCRIPT "${CMAKE_CURRENT_LIST_DIR}/EmbedFile.cmake")

function(project_embed target)
  set(listing "")
  set(outputs "")
  foreach(entry IN LISTS ARGN)
    string(REPLACE "=" ";" parts "${entry}")
    list(GET parts 0 symbol)
    list(GET parts 1 source)
    get_filename_component(source "${source}" ABSOLUTE BASE_DIR "${PROJECT_SOURCE_DIR}")
    set(output "${CMAKE_CURRENT_BINARY_DIR}/embedded/${symbol}.cpp")
    add_custom_command(
      OUTPUT "${output}"
      COMMAND "${CMAKE_COMMAND}" -DSYMBOL=${symbol} "-DSOURCE=${source}" "-DOUTPUT=${output}" -P "${PROJECT_EMBED_SCRIPT}"
      DEPENDS "${source}" "${PROJECT_EMBED_SCRIPT}"
      VERBATIM)
    list(APPEND outputs "${output}")
  endforeach()
  target_sources(${target} PRIVATE ${outputs})
endfunction()
