package service

import (
 "testing"
 "github.com/sonix-devs/netauto/internal/db"
)
func testService(t *testing.T)*Service{t.Helper();d,e:=db.Open(":memory:");if e!=nil{t.Fatal(e)};t.Cleanup(func(){d.Close()});return New(d)}
func TestDiffAddRemove(t *testing.T){d:=CompareConfigurations("a\nb\n","a\nc\n");if d.Added!=1||d.Removed!=1||d.Unchanged!=1{t.Fatalf("unexpected diff: %+v",d)}}
func TestDiffEmpty(t *testing.T){d:=CompareConfigurations("","x");if d.Added!=1||d.Removed!=0{t.Fatalf("unexpected diff: %+v",d)}}
func TestDiffUnified(t *testing.T){d:=CompareConfigurations("old","new");if d.Unified()!="-old\n+new\n"{t.Fatalf("unexpected unified diff %q",d.Unified())}}
func TestSettingRoundTrip(t *testing.T){s:=testService(t);if e:=s.SetSetting("region","lab");e!=nil{t.Fatal(e)};v,e:=s.Setting("region");if e!=nil||v!="lab"{t.Fatalf("got %q, %v",v,e)}}
func TestSettingRejectsEmptyKey(t *testing.T){if e:=testService(t).SetSetting(" ","v");e==nil{t.Fatal("expected validation error")}}
func TestAddJobValidation(t *testing.T){s:=testService(t);if _,e:=s.AddJobWithValidation("job","description","show version");e!=nil{t.Fatal(e)}}
func TestAddJobRejectsBlankCommand(t *testing.T){if _,e:=testService(t).AddJobWithValidation("job",""," ");e==nil{t.Fatal("expected validation error")}}
func TestScheduleCronShape(t *testing.T){if !validCronShape("*/5 * * * 1-5"){t.Fatal("valid cron rejected")};if validCronShape("every day"){t.Fatal("invalid cron accepted")}}
