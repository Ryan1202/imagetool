#ifdef __linux__

#include <dirent.h>

#define FILE_NAME(ptr) ((ptr)->d_name)
#define FILE_ATTR(ptr) ((ptr)->d_type)
#define FILE_ATTR_DIR  (DT_DIR)
#define FILE_ATTR_FILE (DT_REG)

#elif _WIN32

#include <Windows.h>
#undef UNICODE

#define FILE_NAME(ptr) ((char *)&(ptr).cFileName)
#define FILE_ATTR(ptr) ((ptr).dwFileAttributes)
#define FILE_ATTR_DIR  (FILE_ATTRIBUTE_DIRECTORY)
#define FILE_ATTR_FILE (FILE_ATTRIBUTE_ARCHIVE)

#endif