# Bunner Router 리팩토링 기획서

## 1. 개요

`bunner-router-rs`는 고성능 HTTP 라우팅을 위한 라우터, radix 트리, 패턴 매칭 로직을 제공한다. 리팩토링 목적은 **단일 책임 원칙**, **클린 코드**, **안정적인 모듈 의존성**을 지키면서 기존의 모든 기능·비즈니스 로직·알고리즘·옵션·결과값을 전부 유지하는 것이다.

## 2. 설계 원칙

- **단일 책임**: 모듈은 한 가지 개념에만 집중한다. 상태·알고리즘·구성을 기능 단위로 나누어 응집도를 높인다.
- **의존성 제어**: 상위 모듈이 하위 구현에 직접 묶이지 않도록 인터페이스와 데이터 구조를 통해 방향성을 단방향으로 유지한다.
- **기능 보존**: 공개 API 시그니처, `RouterOptions` 기본값, radix 트리 알고리즘, 패턴 파서, 경로 정규화, 오류 코드, RouteMatch 포맷을 변경하지 않는다.
- **확장성**: 옵션·진단·매칭 전략 추가가 쉬워지도록 모듈 간 결합을 최소화한다.

## 3. 단일 책임 모듈 개편안

리팩토링은 라우터 기능을 **공개 접근**, **등록·검증**, **경로 해석**, **매칭 실행**, **Radix 엔진**, **읽기 전용 스냅샷**, **진단**, **공용 데이터**, **메모리 도구**, **동시성 보조** 등 세밀한 책임 단위로 쪼개는 것이 핵심이다. 각 모듈은 한 가지 역할만 담당하며, 나머지 모듈과는 명확한 인터페이스로 소통한다.

### 3.1. 주요 모듈 책임

- **router_access**: `Router`, `RouterOptions`, `RouterReadOnly` 등 외부에 노출되는 퍼사드를 담당한다. 입력 검증, 옵션 해석, 라우터 상태 전환(등록 → 봉인)을 조율한다.
- **route_registry**: 라우트 추가, 벌크 삽입, 옵션에 따른 미리 계산 작업(정적 맵 생성 요청 등)을 처리한다. 라우터 내부 상태를 준비하고 변경 추적을 담당한다.
- **path_analysis**: 경로 정규화·검증, 슬래시 처리, 경로 길이 제한 등 순수 함수 기반의 전처리를 제공한다.
- **pattern_matching**: 세그먼트 토큰 정의, 패턴 파서, 패턴 매처, wildcard 처리, 매칭 점수 계산 로직을 포함한다.
- **match_execution**: path/pattern 분석 결과와 Radix 엔진을 결합해 최종 `RouteMatch`를 계산한다. 우선순위 정책 적용과 파라미터 버퍼 관리를 담당한다.
- **radix_engine**: Radix 트리 자료구조의 빌드, 삽입, 탐색, 압축, 메모리 관리, 정적 맵 유지 등을 전담한다.
- **readonly_snapshot**: `RouterReadOnly` 스냅샷 생성 및 조회, TLS 파라미터 버퍼 관리, 읽기 전용 노드 구조체를 제공한다.
- **diagnostics**: 오류 코드, 오류 객체, 로깅/추적 메타데이터 생성을 담당한다. JSON 포맷 메타데이터도 여기서 유지한다.
- **shared_data**: 공용 타입 별칭, DTO, HTTP 메서드 enum 등 모듈 간 공유되는 단순 데이터 정의를 보관한다.
- **memory_tools**: 문자열 인터너 등 재사용되는 메모리 보조 도구를 담는다.
- **concurrency_tools**: `RwLock`, `OnceLock`, TLS 헬퍼 등 동시성 보조 도구를 모은다.

### 3.2. 디렉터리 구조

```
src/
├── lib.rs
├── router_access/
│   ├── mod.rs             # 공개 엔트리포인트 및 re-export
│   ├── facade.rs          # Router 퍼사드 본체
│   ├── options.rs         # RouterOptions 정의 및 기본값
│   ├── lifecycle.rs       # add/add_bulk/seal 흐름 조율
│   └── readonly_api.rs    # RouterReadOnly 공개 API 래퍼
├── route_registry/
│   ├── mod.rs
│   ├── builder.rs         # 라우트 등록 상태 관리
│   ├── validator.rs       # 경로/메서드 중복 및 제한 검증
│   └── metrics.rs         # 라우트 수, 옵션에 따른 사전 계산 추적
├── path_analysis/
│   ├── mod.rs
│   ├── normalizer.rs      # normalize_and_validate_path 유지
│   └── policies.rs        # 경로 길이, 문자 제한 등 정책
├── pattern_matching/
│   ├── mod.rs
│   ├── lexer.rs
│   ├── matcher.rs
│   └── scoring.rs         # pattern_score, priority 계산
├── match_execution/
│   ├── mod.rs
│   ├── resolver.rs        # Radix 탐색과 패턴 매칭 결합
│   └── param_buffer.rs    # TLS 파라미터 버퍼 및 반환 형식 준비
├── radix_engine/
│   ├── mod.rs             # RadixTree 진입점
│   ├── builder.rs
│   ├── insert.rs
│   ├── traversal.rs
│   ├── compression.rs
│   ├── node.rs
│   ├── memory.rs
│   ├── indices.rs
│   ├── alloc.rs
│   └── static_map.rs
├── readonly_snapshot/
│   ├── mod.rs
│   ├── converter.rs       # Router → ReadOnlyNode 변환
│   └── snapshot.rs        # ReadOnlyNode 구조체 및 조회 로직
├── diagnostics/
│   ├── mod.rs
│   ├── codes.rs           # RouterErrorCode
│   └── error.rs           # RouterError, RouterResult 생성
├── shared_data/
│   ├── mod.rs
│   ├── aliases.rs         # ErrorCode, StaticString, WorkerId, RouteMatch
│   └── enums.rs           # HttpMethod 등 공용 enum
├── memory_tools/
│   ├── mod.rs
│   └── interner.rs        # 문자열 인터너
├── concurrency_tools/
│   ├── mod.rs
│   └── sync.rs            # 동시성 헬퍼, OnceLock 래퍼 등
└── macros.rs (옵션)        # 반복 매크로가 있다면 분리
```

### 3.3. 기존 파일 → 신규 모듈 매핑

| 기존 파일/디렉터리 | 이동 후 위치 | 책임 모듈 | 비고 |
| --- | --- | --- | --- |
| `lib.rs` | `src/lib.rs` + `src/router_access/mod.rs` | router_access | 외부 공개 API는 `router_access`에서 정의하고 `lib.rs`가 재노출한다. |
| `structures.rs` | `router_access/facade.rs`, `router_access/lifecycle.rs` | router_access | Router 퍼사드 구현과 상태 전환 로직을 분리한다. |
| `path.rs` | `path_analysis/normalizer.rs` | path_analysis | 함수/헬퍼를 세분화하되 로직은 그대로 이동한다. |
| `pattern.rs` | `pattern_matching/lexer.rs`, `pattern_matching/matcher.rs`, `pattern_matching/scoring.rs` | pattern_matching | 토큰화·매칭·점수 계산을 각각 파일로 분리한다. |
| `radix_tree.rs` | `radix_engine/mod.rs` | radix_engine | 퍼사드 역할만 남기고 세부 구현은 하위 모듈로 이동한다. |
| `radix_tree/alloc.rs` 등 | `radix_engine/alloc.rs` 등 동일 파일명 | radix_engine | 파일명 유지, 모듈 경로만 조정한다. |
| `readonly.rs` | `readonly_snapshot/converter.rs`, `readonly_snapshot/snapshot.rs`, `match_execution/param_buffer.rs` | readonly_snapshot / match_execution | 스냅샷 변환과 버퍼 관리를 책임 별로 나눈다. |
| `structures.rs` 내 `RouterError` | `diagnostics/error.rs` | diagnostics | 오류 메타데이터 생성을 전담한다. |
| `errors.rs` | `diagnostics/codes.rs` | diagnostics | 코드 enum 정의를 유지한다. |
| `types.rs` | `shared_data/aliases.rs` | shared_data | 타입 별칭을 재배치한다. |
| `enums.rs` | `shared_data/enums.rs` | shared_data | HTTP 메서드 enum을 재배치한다. |
| `interner.rs` | `memory_tools/interner.rs` | memory_tools | 문자열 인터너를 독립 모듈로 둔다. |
| TLS 파라미터 버퍼 관련 코드 | `match_execution/param_buffer.rs` | match_execution | Thread-local 버퍼 관리 책임을 분리한다. |

### 3.4. 공개 API 및 옵션 호환 전략

- `router_access::facade`가 기존 `Router` 메서드를 동일 시그니처로 제공하도록 유지하고, `lib.rs`에서 `pub use router_access::Router;` 형태로 재노출한다.
- `RouterOptions` 기본값과 옵션 필드는 `router_access::options`에 두고, 필요한 입력을 `route_registry`, `match_execution`, `radix_engine`에 주입한다.
- `RouterReadOnly`, `RouteMatch`, `RouterResult`, `RouterError`, `RouterErrorCode`는 각각 `router_access::readonly_api`, `shared_data::aliases`, `diagnostics::error`, `diagnostics::codes`에 위치시키고, 외부 경로(`bunner_router_rs::Router` 등)가 변하지 않도록 `router_access`에서 재노출한다.
- 모듈 이동 시 가시성은 기본적으로 `pub(crate)` 또는 `pub(super)`로 제한해 외부 API가 필요한 표면만 공개한다.

### 3.5. 알고리즘 및 비즈니스 로직 보전

- **경로 정규화/검증**: `normalize_and_validate_path` 함수와 헬퍼를 `path_analysis::normalizer`로 옮기되 구현은 그대로 복사한다.
- **패턴 파서/매처**: 파서, 매처, 점수 계산, wildcard 처리 로직을 `pattern_matching` 하위 파일로 분리하지만 알고리즘은 변경하지 않는다.
- **매칭 실행**: 기존 `RouterReadOnly` 및 매칭 로직이 사용하던 TLS 버퍼, 우선순위 정책을 `match_execution`에서 유지하고 함수 호출 순서를 동일하게 보존한다.
- **Radix 트리**: 삽입, 빌드, 압축, 정적 맵핑, 노드 구조체 코드를 `radix_engine`으로 옮기고 시그니처와 제네릭 제약을 그대로 유지한다.
- **진단 및 타입**: 오류 코드/객체, 타입 별칭, enum 정의는 파일 경로만 변경하고 직렬화 메타데이터와 JSON 포맷을 유지한다.

## 4. 리팩토링 단계

1. **단일 책임 디렉터리 생성**: `src/router_access`, `src/route_registry`, `src/path_analysis`, `src/pattern_matching`, `src/match_execution`, `src/radix_engine`, `src/readonly_snapshot`, `src/diagnostics`, `src/shared_data`, `src/memory_tools`, `src/concurrency_tools` 디렉터리와 각 `mod.rs` 파일을 생성한다.
2. **모듈 선언 재구성**: `lib.rs`에서 새 모듈을 선언하고, 각 모듈 내부에서 필요한 하위 모듈만 `pub(crate)` 또는 `pub(super)`로 노출한다.
3. **코드 자산 이동**: 기존 파일을 책임에 맞는 디렉터리로 옮기고 `use` 경로를 조정한다. 함수 본문과 알고리즘은 수정하지 않는다.
4. **퍼사드-하위 모듈 연결**: `router_access`가 `route_registry`, `path_analysis`, `match_execution`, `radix_engine`을 호출하도록 의존성을 재배치하면서 기존 `Router` 시그니처와 반환 타입을 유지한다.
5. **옵션 전파 흐름 재확인**: `RouterOptions` 초기화 코드를 `router_access::options`에 두고, 필요한 플래그를 `route_registry`, `match_execution`, `radix_engine`에 명시적으로 전달한다.
6. **공용 자산 분산 배치**: 타입 별칭과 enum은 `shared_data`, 오류 관련 코드는 `diagnostics`, 문자열 인터너는 `memory_tools`, 동시성 헬퍼는 `concurrency_tools`, 읽기 전용 스냅샷 구조는 `readonly_snapshot`으로 이동시킨다.

## 5. 결론

이 리팩토링을 통해 Rust 컨벤션을 따르는 보다 체계적이고 유지보수하기 쉬운 코드베이스가 될 것입니다. 관련 코드를 모듈로 그룹화함으로써 개발자 경험을 개선하고 향후 새로운 기능을 더 쉽게 추가할 수 있습니다.
