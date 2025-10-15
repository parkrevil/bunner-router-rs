# bunner-router-rs 기능 확장 마스터 플랜

본 문서는 라우터를 고표현력 매칭 엔진으로 발전시키기 위해 도입해야 할 **패턴 기능**과 **옵션 세트(전역/라우트/파서/런타임/편의)**를 단계적으로 구현하는 로드맵을 정의한다. 모든 단계는 `route-key-only` 철학, 후방 호환성, 성능 회귀 방지를 공통 전제로 삼는다.

## 0. 범위와 원칙

### 포함 범위
- 패턴 표현 확장: 정규식·세그먼트 제약, 옵셔널 세그먼트, 반복(멀티) 세그먼트.
- 설정 계층 정비: 전역(Global), 라우트(Route), 파서(Parser), 런타임(Runtime) 옵션 전부.
- 런타임 행위: 파라미터 추출/타입 변환, fallback 정책, 캐시.
- 개발자 편의: inspect/compile/tokens/test/toRegex/reverse.

### 제외 범위
- 핸들러 저장/미들웨어, 응답 처리 등 상위 프레임워크 기능.
- URL 빌더/문서화 자동화 등 라우팅 외관심사.
- 다국어, 퍼센트 디코딩 정책 재정립(옵션 노출까지만 수행).

## 1. 단계 개요

| Phase | 제목 | 핵심 구현 항목 | 주요 산출물 | 선행 조건 |
|-------|------|----------------|-------------|-----------|
| P1 | 설정 계층 수립 | RouterConfig/RouteOptions/ParserOptions 모델링 | 옵션 구조체, 기본값/검증, 호환 어댑터 | 없음 |
| P2 | 경로 전처리 & 전역 옵션 | case_sensitive, trailing slash, normalizer, 캐시 뼈대 | Preprocess 파이프라인, 캐시 기본, 옵션 테스트 | P1 |
| P3 | 패턴 AST & 파서 확장 | 정규식·옵셔널·반복·파서 고급 옵션 | Pattern AST, Parser, 검증 테스트 | P2 |
| P4 | 매칭 엔진 & 런타임 옵션 | match_order, repeat_match_mode, fallback, 파라미터 추출 | 라딕스 트리 갱신, 런타임 파이프라인, 성능 검증 | P3 |
| P5 | 개발자 편의 기능 | inspect/test/compile/tokens/toRegex/reverse | 고급 API, 예제, 안정성 표기 | P4 |

각 Phase 종료 조건: 해당 기능이 반영된 통합 테스트 시나리오가 통과할 것.

## 2. Phase 상세 계획

### Phase 1. 설정 계층 수립
- **목표**: 모든 옵션을 담는 명확한 설정 계층 정의 및 기본값 확정.
- **커버 옵션**
  - Global: `case_sensitive`, `strict_trailing_slash`, `decode_uri`, `normalize_path`, `allow_duplicate_slash`, `match_order`, `param_pattern_default`, `max_param_depth`, `cache_routes`, `debug`.
  - Route-level: `pattern`, `methods`, `constraints`, `optional`, `repeatable`, `priority`, `meta`, `alias`.
  - Parser: `allow_regex_in_param`, `allow_nested_optional`, `allow_repeat_in_optional`, `param_style`, `escape_chars`, `validate_regex_syntax`.
- **작업**
  - `RouterConfig`, `RouteOptions`, `ParserOptions` 및 Builder/serde 지원.
  - 기본값 테이블과 유효성 검사 로직(예: `priority` 범위, `alias` 중복 금지).
  - 기존 `Router::new(Some(RouterOptions))`를 새 구조로 브리지.
- **Exit Criteria**: 설정 조합별 동작을 검증하는 통합 테스트가 통과.
- **리스크**: API 복잡도 → Builder 패턴, serde config 파일 지원.

### Phase 2. 경로 전처리 & 전역 옵션 구현
- **목표**: 입력 경로를 일관되게 정규화하고 전역 옵션을 반영.
- **커버 옵션**: `case_sensitive`, `strict_trailing_slash`, `decode_uri`, `normalize_path`, `allow_duplicate_slash`, `param_pattern_default`, `cache_routes`(초기화), `debug`.
- **작업**
  - 전처리 파이프라인(`preprocess::apply(path, config)`) 모듈화.
  - 대소문자/슬래시 정책에 맞춘 캐시 키 전략 수립.
  - 기본 세그먼트 패턴(`param_pattern_default`) 적용 경로 작성.
  - LRU 캐시(스켈레톤) 도입 및 `cache_routes` toggle.
  - `debug` flag에 따른 트레이싱 필드 확장.
- **Exit Criteria**: 경로 전처리와 캐시 정책을 검증하는 통합 테스트가 통과.
- **리스크**: 기존 정규화와 충돌 → 기본값을 현행과 동일 유지, 회귀 테스트 강화.

### Phase 3. 패턴 AST & 파서 확장
- **목표**: 정규식/옵셔널/반복 세그먼트를 표현하는 AST와 파서를 완성.
- **커버 옵션**: `allow_regex_in_param`, `allow_nested_optional`, `allow_repeat_in_optional`, `repeatable`, `optional`, `param_style`, `escape_chars`, `validate_regex_syntax`.
- **작업**
  - `PatternAst` 정의(Literal, Param{constraint, repeat}, Group{optional, children}, Wildcard).
  - 파서 리팩터: 스타일 변환(`:id` ↔ `{id}`), escape 처리, 중첩/반복 허용 여부 적용.
  - 정규식 검증(`validate_regex_syntax`)과 constraints vs inline precedence 명문화.
- **Exit Criteria**: 다양한 패턴 시나리오를 검증하는 통합 테스트가 통과.
- **리스크**: 파서 복잡도 → 명시적 상태기계 또는 PEG 도구 도입 검토.

### Phase 4. 매칭 엔진 & 런타임 옵션
- **목표**: 확장된 패턴을 라딕스 트리에 통합하고 런타임 옵션을 구현.
- **커버 옵션/기능**
  - Global: `match_order`, `max_param_depth`.
  - Route-level: `priority`, `alias`.
  - Parser: `repeat_match_mode`.
  - Runtime: `extract_params`, `decode_params`, `coerce_types`, `match_fallback`, `wildcard_param_name`, `segment_validator`.
- **작업**
  - 라딕스 노드 구조 확장: 반복/옵셔널 분기, alias/priority 반영.
  - 매칭 우선순위(`specific-first` vs `defined-first`), `repeat_match_mode`(greedy/lazy) 구현.
  - `max_param_depth` enforcement와 경고/에러 정책 정의.
  - 파라미터 추출/디코딩/타입 변환 파이프라인 구현.
  - fallback 정책(`nearest`, `none`, `default`)과 기본 라우트 지원.
  - `segment_validator` 훅 호출 시점 명시.
- **Exit Criteria**: 복합 매칭·fallback·타입 변환을 검증하는 통합 테스트가 통과.
- **리스크**: 트리 폭증, 성능 저하 → 캐시, pruning, lazy evaluation 적용.

### Phase 5. 개발자 편의 기능
- **목표**: 운영/디버깅/역방향 DX를 강화.
- **기능**: `router.inspect()`, `router.compile(pattern)`, `router.tokens(pattern)`, `router.test(path)`, `router.toRegex(pattern)`, `router.reverse(name, params)`.
- **작업**
  - AST/트리 시각화 serializer(JSON/text) 도입.
  - 파서 결과 노출 및 정규식 변환 API 구현.
  - 런타임 시뮬레이션(`test`) 리포트 포맷 정의.
  - Route naming(`RouteOptions.meta.name`)과 reverse 매핑 로직.
- **Exit Criteria**: 고급 API 시나리오를 검증하는 통합 테스트가 통과.
- **리스크**: 공개 API 증가 → Beta 태그, 안정성 정책 명시.

## 3. 교차 작업 & 의존성
- 통합 테스트: 각 Phase에서 추가된 기능은 통합 테스트 시나리오에 즉시 편입하여 회귀를 방지한다.
- 피드백 루프: `allow_nested_optional`, `repeat_match_mode=lazy` 등은 Feature Flag/RFC로 통제.
- 호환성: 기존 API는 Deprecation 경고를 통해 단계적으로 폐기한다.

## 4. 주요 리스크 및 대응
- **복잡도 폭증**: 단계별 Feature Flag, 명확한 옵션 조합 검증.
- **성능 회귀**: 캐시·프루닝·지연 평가 같은 런타임 전략을 통해 성능을 보호.
- **옵션 조합 폭발**: Validation Layer 강화로 지원/비지원 조합을 코드 수준에서 차단.
- **DX 기대치 상승**: Phase 5 기능으로 실용적인 디버깅·역방향 API를 제공.

---
이 마스터 플랜을 따르면, 라우팅 표현력·설정·런타임·DX 영역을 순차적으로 강화하면서도 통합 테스트를 통해 단계별 기능 완성도를 확인할 수 있다.
