package service

import "testing"

func TestDeviceValidationCase01(t *testing.T) { if err:=ValidateDeviceInput("router-1","mock","192.0.2.2"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase02(t *testing.T) { if err:=ValidateDeviceInput("router-2","mock","192.0.2.3"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase03(t *testing.T) { if err:=ValidateDeviceInput("router-3","mock","192.0.2.4"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase04(t *testing.T) { if err:=ValidateDeviceInput("router-4","mock","192.0.2.5"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase05(t *testing.T) { if err:=ValidateDeviceInput("router-5","mock","192.0.2.6"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase06(t *testing.T) { if err:=ValidateDeviceInput("router-6","mock","192.0.2.7"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase07(t *testing.T) { if err:=ValidateDeviceInput("router-7","mock","192.0.2.8"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase08(t *testing.T) { if err:=ValidateDeviceInput("router-8","mock","192.0.2.9"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase09(t *testing.T) { if err:=ValidateDeviceInput("router-9","mock","192.0.2.10"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase10(t *testing.T) { if err:=ValidateDeviceInput("router-10","mock","192.0.2.11"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase11(t *testing.T) { if err:=ValidateDeviceInput("router-11","mock","192.0.2.12"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase12(t *testing.T) { if err:=ValidateDeviceInput("router-12","mock","192.0.2.13"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase13(t *testing.T) { if err:=ValidateDeviceInput("router-13","mock","192.0.2.14"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase14(t *testing.T) { if err:=ValidateDeviceInput("router-14","mock","192.0.2.15"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase15(t *testing.T) { if err:=ValidateDeviceInput("router-15","mock","192.0.2.16"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase16(t *testing.T) { if err:=ValidateDeviceInput("router-16","mock","192.0.2.17"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase17(t *testing.T) { if err:=ValidateDeviceInput("router-17","mock","192.0.2.18"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase18(t *testing.T) { if err:=ValidateDeviceInput("router-18","mock","192.0.2.19"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase19(t *testing.T) { if err:=ValidateDeviceInput("router-19","mock","192.0.2.20"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase20(t *testing.T) { if err:=ValidateDeviceInput("router-20","mock","192.0.2.21"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase21(t *testing.T) { if err:=ValidateDeviceInput("router-21","mock","192.0.2.22"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase22(t *testing.T) { if err:=ValidateDeviceInput("router-22","mock","192.0.2.23"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase23(t *testing.T) { if err:=ValidateDeviceInput("router-23","mock","192.0.2.24"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase24(t *testing.T) { if err:=ValidateDeviceInput("router-24","mock","192.0.2.25"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase25(t *testing.T) { if err:=ValidateDeviceInput("router-25","mock","192.0.2.26"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase26(t *testing.T) { if err:=ValidateDeviceInput("router-26","mock","192.0.2.27"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase27(t *testing.T) { if err:=ValidateDeviceInput("router-27","mock","192.0.2.28"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase28(t *testing.T) { if err:=ValidateDeviceInput("router-28","mock","192.0.2.29"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase29(t *testing.T) { if err:=ValidateDeviceInput("router-29","mock","192.0.2.30"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase30(t *testing.T) { if err:=ValidateDeviceInput("router-30","mock","192.0.2.31"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase31(t *testing.T) { if err:=ValidateDeviceInput("router-31","mock","192.0.2.32"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase32(t *testing.T) { if err:=ValidateDeviceInput("router-32","mock","192.0.2.33"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase33(t *testing.T) { if err:=ValidateDeviceInput("router-33","mock","192.0.2.34"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase34(t *testing.T) { if err:=ValidateDeviceInput("router-34","mock","192.0.2.35"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase35(t *testing.T) { if err:=ValidateDeviceInput("router-35","mock","192.0.2.36"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase36(t *testing.T) { if err:=ValidateDeviceInput("router-36","mock","192.0.2.37"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase37(t *testing.T) { if err:=ValidateDeviceInput("router-37","mock","192.0.2.38"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase38(t *testing.T) { if err:=ValidateDeviceInput("router-38","mock","192.0.2.39"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase39(t *testing.T) { if err:=ValidateDeviceInput("router-39","mock","192.0.2.40"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase40(t *testing.T) { if err:=ValidateDeviceInput("router-40","mock","192.0.2.41"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase41(t *testing.T) { if err:=ValidateDeviceInput("router-41","mock","192.0.2.42"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase42(t *testing.T) { if err:=ValidateDeviceInput("router-42","mock","192.0.2.43"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase43(t *testing.T) { if err:=ValidateDeviceInput("router-43","mock","192.0.2.44"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase44(t *testing.T) { if err:=ValidateDeviceInput("router-44","mock","192.0.2.45"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase45(t *testing.T) { if err:=ValidateDeviceInput("router-45","mock","192.0.2.46"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase46(t *testing.T) { if err:=ValidateDeviceInput("router-46","mock","192.0.2.47"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase47(t *testing.T) { if err:=ValidateDeviceInput("router-47","mock","192.0.2.48"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase48(t *testing.T) { if err:=ValidateDeviceInput("router-48","mock","192.0.2.49"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase49(t *testing.T) { if err:=ValidateDeviceInput("router-49","mock","192.0.2.50"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase50(t *testing.T) { if err:=ValidateDeviceInput("router-50","mock","192.0.2.51"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase51(t *testing.T) { if err:=ValidateDeviceInput("router-51","mock","192.0.2.52"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase52(t *testing.T) { if err:=ValidateDeviceInput("router-52","mock","192.0.2.53"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase53(t *testing.T) { if err:=ValidateDeviceInput("router-53","mock","192.0.2.54"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase54(t *testing.T) { if err:=ValidateDeviceInput("router-54","mock","192.0.2.55"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase55(t *testing.T) { if err:=ValidateDeviceInput("router-55","mock","192.0.2.56"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase56(t *testing.T) { if err:=ValidateDeviceInput("router-56","mock","192.0.2.57"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase57(t *testing.T) { if err:=ValidateDeviceInput("router-57","mock","192.0.2.58"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase58(t *testing.T) { if err:=ValidateDeviceInput("router-58","mock","192.0.2.59"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase59(t *testing.T) { if err:=ValidateDeviceInput("router-59","mock","192.0.2.60"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestDeviceValidationCase60(t *testing.T) { if err:=ValidateDeviceInput("router-60","mock","192.0.2.61"); err!=nil { t.Fatalf("valid device rejected: %v",err) } }
func TestRejectInvalidAdapter(t *testing.T) { if ValidateDeviceInput("r","telnet","host")==nil { t.Fatal("unsupported adapter accepted") } }
func TestRejectEmptyDeviceName(t *testing.T) { if ValidateDeviceInput(" ","mock","host")==nil { t.Fatal("empty name accepted") } }
func TestRejectEmptyHost(t *testing.T) { if ValidateDeviceInput("r","mock"," ")==nil { t.Fatal("empty host accepted") } }
func TestRejectHostLineBreak(t *testing.T) { if ValidateDeviceInput("r","mock","host\nshutdown")==nil { t.Fatal("line break accepted") } }
func TestRejectHostNull(t *testing.T) { if ValidateDeviceInput("r","mock","host\x00")==nil { t.Fatal("NUL accepted") } }
