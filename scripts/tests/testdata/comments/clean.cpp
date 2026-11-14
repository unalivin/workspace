#include <string>
const char* a = R"sql(SELECT 1 -- kept // kept)sql";
int b = 1'000 / 2;
const char* c = "http://example.test";
char d = '"';
const char* e = R"(/* raw */)";
