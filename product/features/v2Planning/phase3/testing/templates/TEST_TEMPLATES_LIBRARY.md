# Neural Trader V2 - Test Templates Library

## Overview

Comprehensive collection of test templates for Neural Trader V2 to ensure consistent, thorough testing across all components and scenarios.

## Template Categories

### 1. Unit Test Templates
### 2. Integration Test Templates
### 3. E2E Test Templates
### 4. Performance Test Templates
### 5. Security Test Templates
### 6. API Test Templates

## 1. Unit Test Templates

### Service Layer Unit Test Template
```typescript
// tests/templates/service-unit-test.template.ts
import { describe, beforeEach, afterEach, it, expect, jest } from '@jest/globals';
import { MockRepository } from '../mocks/mock-repository';
import { TestDataFactory } from '../generators/test-data-factory';

// Template: Service Unit Test
describe('[SERVICE_NAME]Service', () => {
  let service: [SERVICE_NAME]Service;
  let mockRepository: jest.Mocked<[REPOSITORY_NAME]Repository>;
  let testDataFactory: TestDataFactory;

  beforeEach(() => {
    // Arrange - Setup mocks and dependencies
    mockRepository = new MockRepository() as jest.Mocked<[REPOSITORY_NAME]Repository>;
    testDataFactory = new TestDataFactory({ seed: 42, realistic: true });
    
    service = new [SERVICE_NAME]Service(mockRepository);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('[METHOD_NAME]', () => {
    // Happy path test
    it('should [EXPECTED_BEHAVIOR] when [VALID_INPUT_CONDITION]', async () => {
      // Arrange
      const inputData = testDataFactory.generate[INPUT_TYPE]();
      const expectedResult = testDataFactory.generate[EXPECTED_TYPE]();
      mockRepository.[MOCK_METHOD].mockResolvedValue(expectedResult);

      // Act
      const result = await service.[METHOD_NAME](inputData);

      // Assert
      expect(result).toEqual(expectedResult);
      expect(mockRepository.[MOCK_METHOD]).toHaveBeenCalledWith(inputData);
      expect(mockRepository.[MOCK_METHOD]).toHaveBeenCalledTimes(1);
    });

    // Error handling test
    it('should throw [ERROR_TYPE] when [ERROR_CONDITION]', async () => {
      // Arrange
      const invalidInput = testDataFactory.generate[INVALID_TYPE]();
      mockRepository.[MOCK_METHOD].mockRejectedValue(new [ERROR_TYPE]('[ERROR_MESSAGE]'));

      // Act & Assert
      await expect(service.[METHOD_NAME](invalidInput))
        .rejects
        .toThrow([ERROR_TYPE]);
    });

    // Edge case tests
    it('should handle empty input gracefully', async () => {
      // Arrange
      const emptyInput = null;
      
      // Act & Assert
      await expect(service.[METHOD_NAME](emptyInput))
        .rejects
        .toThrow('Input cannot be null or undefined');
    });

    it('should handle boundary values correctly', async () => {
      // Arrange
      const boundaryInput = testDataFactory.generateBoundaryValue();
      const expectedResult = testDataFactory.generateExpectedBoundaryResult();
      mockRepository.[MOCK_METHOD].mockResolvedValue(expectedResult);

      // Act
      const result = await service.[METHOD_NAME](boundaryInput);

      // Assert
      expect(result).toEqual(expectedResult);
    });
  });

  // Performance test
  describe('Performance', () => {
    it('should complete [METHOD_NAME] within performance threshold', async () => {
      // Arrange
      const inputData = testDataFactory.generate[INPUT_TYPE]();
      const performanceThreshold = 100; // ms
      
      // Act
      const startTime = performance.now();
      await service.[METHOD_NAME](inputData);
      const executionTime = performance.now() - startTime;

      // Assert
      expect(executionTime).toBeLessThan(performanceThreshold);
    });
  });
});
```

### Repository Unit Test Template
```typescript
// tests/templates/repository-unit-test.template.ts
describe('[REPOSITORY_NAME]Repository', () => {
  let repository: [REPOSITORY_NAME]Repository;
  let mockDatabase: jest.Mocked<DatabaseConnection>;
  let testDataFactory: TestDataFactory;

  beforeEach(async () => {
    mockDatabase = new MockDatabaseConnection();
    testDataFactory = new TestDataFactory();
    repository = new [REPOSITORY_NAME]Repository(mockDatabase);
  });

  describe('create', () => {
    it('should create new [ENTITY_NAME] with valid data', async () => {
      // Arrange
      const entityData = testDataFactory.generate[ENTITY_NAME]Data();
      const expectedEntity = { id: 'generated-id', ...entityData };
      mockDatabase.query.mockResolvedValue({ rows: [expectedEntity] });

      // Act
      const result = await repository.create(entityData);

      // Assert
      expect(result).toEqual(expectedEntity);
      expect(mockDatabase.query).toHaveBeenCalledWith(
        expect.stringContaining('INSERT INTO'),
        expect.arrayContaining(Object.values(entityData))
      );
    });

    it('should handle database constraint violations', async () => {
      // Arrange
      const duplicateData = testDataFactory.generate[ENTITY_NAME]Data();
      mockDatabase.query.mockRejectedValue(new Error('duplicate key value'));

      // Act & Assert
      await expect(repository.create(duplicateData))
        .rejects
        .toThrow(DuplicateEntityError);
    });
  });

  describe('findById', () => {
    it('should return entity when found', async () => {
      // Arrange
      const entityId = 'test-id';
      const expectedEntity = testDataFactory.generate[ENTITY_NAME]({ id: entityId });
      mockDatabase.query.mockResolvedValue({ rows: [expectedEntity] });

      // Act
      const result = await repository.findById(entityId);

      // Assert
      expect(result).toEqual(expectedEntity);
      expect(mockDatabase.query).toHaveBeenCalledWith(
        expect.stringContaining('SELECT * FROM'),
        [entityId]
      );
    });

    it('should return null when entity not found', async () => {
      // Arrange
      const nonExistentId = 'non-existent';
      mockDatabase.query.mockResolvedValue({ rows: [] });

      // Act
      const result = await repository.findById(nonExistentId);

      // Assert
      expect(result).toBeNull();
    });
  });

  describe('update', () => {
    it('should update existing entity', async () => {
      // Arrange
      const entityId = 'test-id';
      const updateData = testDataFactory.generatePartial[ENTITY_NAME]Data();
      const updatedEntity = { id: entityId, ...updateData };
      mockDatabase.query.mockResolvedValue({ rows: [updatedEntity] });

      // Act
      const result = await repository.update(entityId, updateData);

      // Assert
      expect(result).toEqual(updatedEntity);
      expect(mockDatabase.query).toHaveBeenCalledWith(
        expect.stringContaining('UPDATE'),
        expect.arrayContaining([...Object.values(updateData), entityId])
      );
    });
  });

  describe('delete', () => {
    it('should delete entity by id', async () => {
      // Arrange
      const entityId = 'test-id';
      mockDatabase.query.mockResolvedValue({ rowCount: 1 });

      // Act
      const result = await repository.delete(entityId);

      // Assert
      expect(result).toBe(true);
      expect(mockDatabase.query).toHaveBeenCalledWith(
        expect.stringContaining('DELETE FROM'),
        [entityId]
      );
    });

    it('should return false when entity not found for deletion', async () => {
      // Arrange
      const nonExistentId = 'non-existent';
      mockDatabase.query.mockResolvedValue({ rowCount: 0 });

      // Act
      const result = await repository.delete(nonExistentId);

      // Assert
      expect(result).toBe(false);
    });
  });
});
```

## 2. Integration Test Templates

### Service Integration Test Template
```typescript
// tests/templates/service-integration-test.template.ts
describe('[SERVICE_NAME] Integration', () => {
  let app: Application;
  let database: TestDatabase;
  let testDataSetup: TestDataSetup;
  let authToken: string;

  beforeAll(async () => {
    // Setup test environment
    database = TestDatabase.getInstance();
    await database.setupTestDatabase();
    
    app = createTestApp();
    testDataSetup = new TestDataSetup();
    
    // Setup authentication
    authToken = await createTestAuthToken();
  });

  afterAll(async () => {
    await database.close();
    await app.close();
  });

  beforeEach(async () => {
    await database.cleanDatabase();
  });

  describe('[ENDPOINT_PATH] Integration', () => {
    it('should handle complete [BUSINESS_OPERATION] workflow', async () => {
      // Arrange - Setup test data
      const testScenario = await testDataSetup.setupCompleteTestScenario();
      await testDataSetup.seedDatabase(testScenario);

      // Act - Execute the workflow
      const step1Response = await request(app)
        .post('[FIRST_ENDPOINT]')
        .set('Authorization', `Bearer ${authToken}`)
        .send(testScenario.initialRequest)
        .expect(201);

      const step2Response = await request(app)
        .get(`[SECOND_ENDPOINT]/${step1Response.body.id}`)
        .set('Authorization', `Bearer ${authToken}`)
        .expect(200);

      const finalResponse = await request(app)
        .put(`[FINAL_ENDPOINT]/${step1Response.body.id}`)
        .set('Authorization', `Bearer ${authToken}`)
        .send(testScenario.updateRequest)
        .expect(200);

      // Assert - Verify complete workflow
      expect(finalResponse.body).toMatchObject(testScenario.expectedFinalState);

      // Verify database state
      const dbState = await database.query('SELECT * FROM [TABLE_NAME] WHERE id = $1', [step1Response.body.id]);
      expect(dbState.rows[0]).toMatchObject(testScenario.expectedDbState);

      // Verify side effects
      const auditLogs = await database.query('SELECT * FROM audit_logs WHERE entity_id = $1', [step1Response.body.id]);
      expect(auditLogs.rows).toHaveLength(3); // One for each operation
    });

    it('should handle error scenarios gracefully', async () => {
      // Arrange - Setup error conditions
      const invalidData = testDataSetup.generateInvalidData();

      // Act & Assert - Test error handling
      const errorResponse = await request(app)
        .post('[ENDPOINT_PATH]')
        .set('Authorization', `Bearer ${authToken}`)
        .send(invalidData)
        .expect(400);

      expect(errorResponse.body).toHaveProperty('error');
      expect(errorResponse.body.error).toContain('[EXPECTED_ERROR_MESSAGE]');

      // Verify no side effects occurred
      const dbState = await database.query('SELECT COUNT(*) FROM [TABLE_NAME]');
      expect(Number(dbState.rows[0].count)).toBe(0);
    });

    it('should handle concurrent requests safely', async () => {
      // Arrange
      const concurrentRequests = 10;
      const testData = testDataSetup.generateBatch[REQUEST_TYPE](concurrentRequests);

      // Act - Execute concurrent requests
      const promises = testData.map(data =>
        request(app)
          .post('[ENDPOINT_PATH]')
          .set('Authorization', `Bearer ${authToken}`)
          .send(data)
      );

      const responses = await Promise.all(promises);

      // Assert - Verify all requests succeeded
      responses.forEach(response => {
        expect(response.status).toBe(201);
        expect(response.body).toHaveProperty('id');
      });

      // Verify database consistency
      const dbCount = await database.query('SELECT COUNT(*) FROM [TABLE_NAME]');
      expect(Number(dbCount.rows[0].count)).toBe(concurrentRequests);
    });
  });
});
```

### Database Integration Test Template
```typescript
// tests/templates/database-integration-test.template.ts
describe('[ENTITY_NAME] Database Integration', () => {
  let database: TestDatabase;
  let repository: [ENTITY_NAME]Repository;
  let testDataFactory: TestDataFactory;

  beforeAll(async () => {
    database = TestDatabase.getInstance();
    await database.setupTestDatabase();
    repository = new [ENTITY_NAME]Repository(database.getConnection());
    testDataFactory = new TestDataFactory();
  });

  beforeEach(async () => {
    await database.cleanTable('[TABLE_NAME]');
  });

  afterAll(async () => {
    await database.close();
  });

  describe('CRUD Operations', () => {
    it('should perform complete CRUD lifecycle', async () => {
      // Create
      const entityData = testDataFactory.generate[ENTITY_NAME]Data();
      const createdEntity = await repository.create(entityData);
      
      expect(createdEntity).toHaveProperty('id');
      expect(createdEntity).toMatchObject(entityData);

      // Read
      const retrievedEntity = await repository.findById(createdEntity.id);
      expect(retrievedEntity).toEqual(createdEntity);

      // Update
      const updateData = testDataFactory.generatePartial[ENTITY_NAME]Data();
      const updatedEntity = await repository.update(createdEntity.id, updateData);
      expect(updatedEntity).toMatchObject({ ...createdEntity, ...updateData });

      // Delete
      const deleted = await repository.delete(createdEntity.id);
      expect(deleted).toBe(true);

      // Verify deletion
      const deletedEntity = await repository.findById(createdEntity.id);
      expect(deletedEntity).toBeNull();
    });

    it('should handle database constraints', async () => {
      // Test unique constraints
      const entityData = testDataFactory.generate[ENTITY_NAME]Data();
      await repository.create(entityData);

      // Attempt to create duplicate
      await expect(repository.create(entityData))
        .rejects
        .toThrow(DuplicateEntityError);

      // Test foreign key constraints
      const invalidForeignKey = testDataFactory.generate[ENTITY_NAME]Data({
        [FOREIGN_KEY_FIELD]: 'non-existent-id'
      });

      await expect(repository.create(invalidForeignKey))
        .rejects
        .toThrow(ForeignKeyConstraintError);
    });

    it('should handle transactions correctly', async () => {
      const entities = testDataFactory.generateBatch[ENTITY_NAME](3);

      // Test successful transaction
      await database.transaction(async (trx) => {
        for (const entity of entities) {
          await repository.create(entity, trx);
        }
      });

      const count = await repository.count();
      expect(count).toBe(3);

      // Test rollback on error
      await expect(database.transaction(async (trx) => {
        await repository.create(testDataFactory.generate[ENTITY_NAME]Data(), trx);
        throw new Error('Intentional error');
      })).rejects.toThrow();

      const finalCount = await repository.count();
      expect(finalCount).toBe(3); // No additional entities should be created
    });
  });

  describe('Query Performance', () => {
    beforeEach(async () => {
      // Seed performance test data
      const performanceData = testDataFactory.generateBatch[ENTITY_NAME](1000);
      await Promise.all(
        performanceData.map(data => repository.create(data))
      );
    });

    it('should execute queries within performance thresholds', async () => {
      const performanceThreshold = 100; // ms

      // Test single entity retrieval
      const startTime = performance.now();
      await repository.findById('some-id');
      const singleQueryTime = performance.now() - startTime;
      
      expect(singleQueryTime).toBeLessThan(performanceThreshold);

      // Test batch retrieval
      const batchStartTime = performance.now();
      await repository.findMany({ limit: 100 });
      const batchQueryTime = performance.now() - batchStartTime;
      
      expect(batchQueryTime).toBeLessThan(performanceThreshold * 2);
    });

    it('should use indexes effectively', async () => {
      // Query by indexed field should be fast
      const indexedQueryStart = performance.now();
      await repository.findBy[INDEXED_FIELD]('indexed-value');
      const indexedQueryTime = performance.now() - indexedQueryStart;

      // Query by non-indexed field should be slower but still reasonable
      const nonIndexedQueryStart = performance.now();
      await repository.findBy[NON_INDEXED_FIELD]('non-indexed-value');
      const nonIndexedQueryTime = performance.now() - nonIndexedQueryStart;

      expect(indexedQueryTime).toBeLessThan(50); // ms
      expect(nonIndexedQueryTime).toBeLessThan(500); // ms
    });
  });
});
```

## 3. End-to-End Test Templates

### User Journey E2E Test Template
```typescript
// tests/templates/e2e-user-journey.template.ts
describe('[USER_JOURNEY_NAME] E2E', () => {
  let browser: Browser;
  let page: Page;
  let testDataSetup: TestDataSetup;
  let userContext: UserContext;

  beforeAll(async () => {
    browser = await chromium.launch({ headless: true });
    testDataSetup = new TestDataSetup();
  });

  afterAll(async () => {
    await browser.close();
  });

  beforeEach(async () => {
    page = await browser.newPage();
    userContext = await testDataSetup.setupUserContext();
  });

  afterEach(async () => {
    await page.close();
  });

  it('should complete [USER_JOURNEY_NAME] successfully', async () => {
    // Step 1: User Authentication
    await page.goto('[LOGIN_URL]');
    await page.fill('[USERNAME_SELECTOR]', userContext.username);
    await page.fill('[PASSWORD_SELECTOR]', userContext.password);
    await page.click('[LOGIN_BUTTON_SELECTOR]');
    
    // Verify successful login
    await expect(page.locator('[DASHBOARD_INDICATOR]')).toBeVisible();

    // Step 2: Navigate to Feature
    await page.click('[NAVIGATION_SELECTOR]');
    await expect(page.locator('[FEATURE_PAGE_INDICATOR]')).toBeVisible();

    // Step 3: Perform Core Action
    await page.fill('[INPUT_FIELD_SELECTOR]', userContext.testData.inputValue);
    await page.click('[SUBMIT_BUTTON_SELECTOR]');

    // Verify loading state
    await expect(page.locator('[LOADING_INDICATOR]')).toBeVisible();
    await expect(page.locator('[LOADING_INDICATOR]')).not.toBeVisible();

    // Step 4: Verify Results
    await expect(page.locator('[SUCCESS_MESSAGE]')).toBeVisible();
    await expect(page.locator('[RESULT_DISPLAY]')).toContainText(userContext.expectedResult);

    // Step 5: Verify Side Effects
    await page.click('[SECONDARY_VIEW_SELECTOR]');
    await expect(page.locator('[SIDE_EFFECT_INDICATOR]')).toBeVisible();

    // Step 6: Cleanup/Logout
    await page.click('[USER_MENU_SELECTOR]');
    await page.click('[LOGOUT_SELECTOR]');
    
    // Verify logout
    await expect(page.locator('[LOGIN_FORM]')).toBeVisible();
  });

  it('should handle error scenarios gracefully', async () => {
    // Setup error conditions
    const errorScenario = testDataSetup.generateErrorScenario();

    await page.goto('[FEATURE_URL]');
    
    // Simulate error condition
    await page.fill('[INPUT_FIELD_SELECTOR]', errorScenario.invalidInput);
    await page.click('[SUBMIT_BUTTON_SELECTOR]');

    // Verify error handling
    await expect(page.locator('[ERROR_MESSAGE]')).toBeVisible();
    await expect(page.locator('[ERROR_MESSAGE]')).toContainText(errorScenario.expectedErrorMessage);

    // Verify system recovery
    await page.fill('[INPUT_FIELD_SELECTOR]', errorScenario.validInput);
    await page.click('[SUBMIT_BUTTON_SELECTOR]');
    
    await expect(page.locator('[SUCCESS_MESSAGE]')).toBeVisible();
    await expect(page.locator('[ERROR_MESSAGE]')).not.toBeVisible();
  });

  it('should work across different browsers and devices', async () => {
    // Test responsive design
    const viewports = [
      { width: 1920, height: 1080 }, // Desktop
      { width: 1024, height: 768 },  // Tablet
      { width: 375, height: 667 }    // Mobile
    ];

    for (const viewport of viewports) {
      await page.setViewportSize(viewport);
      await page.reload();

      // Verify responsive behavior
      await expect(page.locator('[RESPONSIVE_ELEMENT]')).toBeVisible();
      
      // Test core functionality still works
      await page.fill('[INPUT_FIELD_SELECTOR]', userContext.testData.inputValue);
      await page.click('[SUBMIT_BUTTON_SELECTOR]');
      await expect(page.locator('[SUCCESS_MESSAGE]')).toBeVisible();
    }
  });
});
```

## 4. Performance Test Templates

### Load Test Template
```typescript
// tests/templates/load-test.template.ts
import { check } from 'k6';
import http from 'k6/http';
import { Rate } from 'k6/metrics';

export let errorRate = new Rate('errors');

export let options = {
  stages: [
    { duration: '1m', target: 10 },   // Warm-up
    { duration: '3m', target: 50 },   // Normal load
    { duration: '2m', target: 100 },  // High load
    { duration: '1m', target: 0 },    // Cool-down
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    errors: ['rate<0.01'],
  },
};

export default function () {
  // [TEST_SCENARIO_NAME] Load Test
  
  // Test data generation
  const testData = {
    [DATA_FIELD]: `test-value-${Math.random()}`,
    // Add other test data fields
  };

  // Authentication (if required)
  const authToken = '[AUTH_TOKEN]';
  const headers = {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${authToken}`,
  };

  // Execute request
  const response = http.post(
    '[API_ENDPOINT]',
    JSON.stringify(testData),
    { headers }
  );

  // Validate response
  const success = check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 200ms': (r) => r.timings.duration < 200,
    'response has data': (r) => r.body.length > 0,
    'response is valid JSON': (r) => {
      try {
        JSON.parse(r.body);
        return true;
      } catch {
        return false;
      }
    },
  });

  errorRate.add(!success);

  // Think time
  sleep(1);
}

export function teardown() {
  // Cleanup after test
  console.log('[TEST_SCENARIO_NAME] load test completed');
}
```

## 5. Security Test Templates

### Authentication Security Test Template
```typescript
// tests/templates/security-auth-test.template.ts
describe('[FEATURE_NAME] Security Tests', () => {
  let app: Application;
  let testDataSetup: TestDataSetup;
  let securityTester: SecurityTester;

  beforeAll(async () => {
    app = createTestApp();
    testDataSetup = new TestDataSetup();
    securityTester = new SecurityTester();
  });

  afterAll(async () => {
    await app.close();
  });

  describe('Authentication Security', () => {
    it('should reject requests without authentication', async () => {
      const response = await request(app)
        .get('[PROTECTED_ENDPOINT]')
        .expect(401);

      expect(response.body).toHaveProperty('error');
      expect(response.body.error).toContain('authentication required');
    });

    it('should reject requests with invalid tokens', async () => {
      const invalidTokens = [
        'invalid-token',
        'Bearer invalid',
        'Bearer ' + 'a'.repeat(1000), // Extremely long token
        'Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.invalid.signature',
      ];

      for (const token of invalidTokens) {
        const response = await request(app)
          .get('[PROTECTED_ENDPOINT]')
          .set('Authorization', token)
          .expect(401);

        expect(response.body).toHaveProperty('error');
      }
    });

    it('should reject expired tokens', async () => {
      const expiredToken = await testDataSetup.generateExpiredAuthToken();

      const response = await request(app)
        .get('[PROTECTED_ENDPOINT]')
        .set('Authorization', `Bearer ${expiredToken}`)
        .expect(401);

      expect(response.body.error).toContain('token expired');
    });

    it('should handle token refresh securely', async () => {
      const refreshToken = await testDataSetup.generateRefreshToken();

      // Valid refresh
      const response = await request(app)
        .post('/auth/refresh')
        .send({ refreshToken })
        .expect(200);

      expect(response.body).toHaveProperty('accessToken');
      expect(response.body).toHaveProperty('expiresIn');

      // Refresh token should be single-use
      await request(app)
        .post('/auth/refresh')
        .send({ refreshToken })
        .expect(401);
    });
  });

  describe('Authorization Security', () => {
    it('should enforce role-based access control', async () => {
      const userToken = await testDataSetup.generateUserToken('regular_user');
      const adminToken = await testDataSetup.generateUserToken('admin');

      // Regular user should be denied admin endpoints
      await request(app)
        .get('[ADMIN_ENDPOINT]')
        .set('Authorization', `Bearer ${userToken}`)
        .expect(403);

      // Admin should have access
      await request(app)
        .get('[ADMIN_ENDPOINT]')
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);
    });

    it('should prevent privilege escalation', async () => {
      const userToken = await testDataSetup.generateUserToken('regular_user');

      // Attempt to modify own role
      await request(app)
        .put('/users/self')
        .set('Authorization', `Bearer ${userToken}`)
        .send({ role: 'admin' })
        .expect(403);

      // Attempt to access other user's data
      const otherUserId = await testDataSetup.getOtherUserId();
      await request(app)
        .get(`/users/${otherUserId}`)
        .set('Authorization', `Bearer ${userToken}`)
        .expect(403);
    });
  });

  describe('Input Validation Security', () => {
    it('should prevent SQL injection attacks', async () => {
      const authToken = await testDataSetup.generateValidAuthToken();
      const sqlInjectionPayloads = [
        "'; DROP TABLE users; --",
        "' OR '1'='1",
        "'; UPDATE users SET role='admin' WHERE id=1; --",
        "' UNION SELECT password FROM users --"
      ];

      for (const payload of sqlInjectionPayloads) {
        const response = await request(app)
          .get('[SEARCH_ENDPOINT]')
          .set('Authorization', `Bearer ${authToken}`)
          .query({ q: payload })
          .expect(400);

        expect(response.body.error).toContain('Invalid input');
      }
    });

    it('should prevent XSS attacks', async () => {
      const authToken = await testDataSetup.generateValidAuthToken();
      const xssPayloads = [
        '<script>alert("XSS")</script>',
        'javascript:alert("XSS")',
        '<img src="x" onerror="alert(\'XSS\')">',
        '<svg onload="alert(\'XSS\')"></svg>'
      ];

      for (const payload of xssPayloads) {
        const response = await request(app)
          .post('[INPUT_ENDPOINT]')
          .set('Authorization', `Bearer ${authToken}`)
          .send({ content: payload });

        // Should either reject or sanitize
        if (response.status === 200) {
          expect(response.body.content).not.toContain('<script');
          expect(response.body.content).not.toContain('javascript:');
        } else {
          expect(response.status).toBe(400);
        }
      }
    });

    it('should prevent CSRF attacks', async () => {
      const authToken = await testDataSetup.generateValidAuthToken();

      // Request without CSRF token should fail
      await request(app)
        .post('[STATE_CHANGING_ENDPOINT]')
        .set('Authorization', `Bearer ${authToken}`)
        .send({ data: 'test' })
        .expect(403);

      // Request with invalid CSRF token should fail
      await request(app)
        .post('[STATE_CHANGING_ENDPOINT]')
        .set('Authorization', `Bearer ${authToken}`)
        .set('X-CSRF-Token', 'invalid-token')
        .send({ data: 'test' })
        .expect(403);
    });
  });

  describe('Rate Limiting Security', () => {
    it('should enforce rate limits', async () => {
      const authToken = await testDataSetup.generateValidAuthToken();
      const rateLimitThreshold = 100; // requests per minute

      // Make requests up to limit
      const requests = Array.from({ length: rateLimitThreshold }, (_, i) =>
        request(app)
          .get('[RATE_LIMITED_ENDPOINT]')
          .set('Authorization', `Bearer ${authToken}`)
      );

      const responses = await Promise.all(requests);
      
      // All requests within limit should succeed
      responses.forEach(response => {
        expect(response.status).toBe(200);
      });

      // Additional request should be rate limited
      await request(app)
        .get('[RATE_LIMITED_ENDPOINT]')
        .set('Authorization', `Bearer ${authToken}`)
        .expect(429);
    });
  });
});
```

## 6. API Contract Test Templates

### API Contract Test Template
```typescript
// tests/templates/api-contract-test.template.ts
describe('[API_NAME] Contract Tests', () => {
  let app: Application;
  let testDataFactory: TestDataFactory;
  let authToken: string;

  beforeAll(async () => {
    app = createTestApp();
    testDataFactory = new TestDataFactory();
    authToken = await generateTestAuthToken();
  });

  afterAll(async () => {
    await app.close();
  });

  describe('GET [ENDPOINT_PATH]', () => {
    it('should return data matching the API schema', async () => {
      const response = await request(app)
        .get('[ENDPOINT_PATH]')
        .set('Authorization', `Bearer ${authToken}`)
        .expect(200)
        .expect('Content-Type', /json/);

      // Validate response structure
      expect(response.body).toHaveProperty('data');
      expect(response.body).toHaveProperty('meta');
      
      if (response.body.data.length > 0) {
        const firstItem = response.body.data[0];
        
        // Validate required fields
        expect(firstItem).toHaveProperty('id');
        expect(firstItem).toHaveProperty('[REQUIRED_FIELD_1]');
        expect(firstItem).toHaveProperty('[REQUIRED_FIELD_2]');
        
        // Validate field types
        expect(typeof firstItem.id).toBe('string');
        expect(typeof firstItem.[REQUIRED_FIELD_1]).toBe('[EXPECTED_TYPE]');
        
        // Validate field formats
        if (firstItem.email) {
          expect(firstItem.email).toMatch(/^[^\s@]+@[^\s@]+\.[^\s@]+$/);
        }
        
        if (firstItem.timestamp) {
          expect(new Date(firstItem.timestamp)).toBeInstanceOf(Date);
        }
      }

      // Validate pagination metadata
      expect(response.body.meta).toHaveProperty('page');
      expect(response.body.meta).toHaveProperty('limit');
      expect(response.body.meta).toHaveProperty('total');
    });

    it('should support pagination parameters', async () => {
      const page = 1;
      const limit = 10;
      
      const response = await request(app)
        .get('[ENDPOINT_PATH]')
        .set('Authorization', `Bearer ${authToken}`)
        .query({ page, limit })
        .expect(200);

      expect(response.body.data.length).toBeLessThanOrEqual(limit);
      expect(response.body.meta.page).toBe(page);
      expect(response.body.meta.limit).toBe(limit);
    });

    it('should support filtering and sorting', async () => {
      const filterValue = 'test-filter';
      const sortField = '[SORTABLE_FIELD]';
      const sortOrder = 'desc';

      const response = await request(app)
        .get('[ENDPOINT_PATH]')
        .set('Authorization', `Bearer ${authToken}`)
        .query({ 
          filter: filterValue,
          sort: sortField,
          order: sortOrder
        })
        .expect(200);

      // Validate filtering
      if (response.body.data.length > 0) {
        response.body.data.forEach(item => {
          expect(item).toMatchObject(
            expect.objectContaining({
              [FILTER_FIELD]: expect.stringContaining(filterValue)
            })
          );
        });

        // Validate sorting
        const values = response.body.data.map(item => item[sortField]);
        const sortedValues = [...values].sort((a, b) => 
          sortOrder === 'desc' ? b.localeCompare(a) : a.localeCompare(b)
        );
        expect(values).toEqual(sortedValues);
      }
    });
  });

  describe('POST [ENDPOINT_PATH]', () => {
    it('should create resource with valid data', async () => {
      const requestData = testDataFactory.generate[RESOURCE_TYPE]Data();

      const response = await request(app)
        .post('[ENDPOINT_PATH]')
        .set('Authorization', `Bearer ${authToken}`)
        .send(requestData)
        .expect(201)
        .expect('Content-Type', /json/);

      // Validate response structure
      expect(response.body).toHaveProperty('id');
      expect(response.body).toMatchObject(requestData);
      expect(response.body).toHaveProperty('createdAt');
      expect(response.body).toHaveProperty('updatedAt');

      // Validate timestamps
      expect(new Date(response.body.createdAt)).toBeInstanceOf(Date);
      expect(new Date(response.body.updatedAt)).toBeInstanceOf(Date);

      // Validate location header
      expect(response.headers.location).toBe(`[ENDPOINT_PATH]/${response.body.id}`);
    });

    it('should validate required fields', async () => {
      const requiredFields = ['[REQUIRED_FIELD_1]', '[REQUIRED_FIELD_2]'];
      
      for (const field of requiredFields) {
        const incompleteData = testDataFactory.generate[RESOURCE_TYPE]Data();
        delete incompleteData[field];

        const response = await request(app)
          .post('[ENDPOINT_PATH]')
          .set('Authorization', `Bearer ${authToken}`)
          .send(incompleteData)
          .expect(400);

        expect(response.body).toHaveProperty('error');
        expect(response.body.error).toContain(`${field} is required`);
      }
    });

    it('should validate field constraints', async () => {
      const constraintTests = [
        {
          field: '[STRING_FIELD]',
          invalidValue: 'x'.repeat(1001), // Too long
          expectedError: 'exceeds maximum length'
        },
        {
          field: '[NUMERIC_FIELD]',
          invalidValue: -1, // Negative when positive required
          expectedError: 'must be positive'
        },
        {
          field: '[EMAIL_FIELD]',
          invalidValue: 'invalid-email',
          expectedError: 'must be valid email'
        }
      ];

      for (const test of constraintTests) {
        const invalidData = testDataFactory.generate[RESOURCE_TYPE]Data();
        invalidData[test.field] = test.invalidValue;

        const response = await request(app)
          .post('[ENDPOINT_PATH]')
          .set('Authorization', `Bearer ${authToken}`)
          .send(invalidData)
          .expect(400);

        expect(response.body.error).toContain(test.expectedError);
      }
    });
  });

  describe('Error Handling', () => {
    it('should return consistent error format', async () => {
      const response = await request(app)
        .get('[ENDPOINT_PATH]/non-existent-id')
        .set('Authorization', `Bearer ${authToken}`)
        .expect(404);

      // Validate error response structure
      expect(response.body).toHaveProperty('error');
      expect(response.body).toHaveProperty('code');
      expect(response.body).toHaveProperty('timestamp');
      
      expect(typeof response.body.error).toBe('string');
      expect(typeof response.body.code).toBe('string');
      expect(new Date(response.body.timestamp)).toBeInstanceOf(Date);
    });

    it('should handle malformed JSON gracefully', async () => {
      const response = await request(app)
        .post('[ENDPOINT_PATH]')
        .set('Authorization', `Bearer ${authToken}`)
        .set('Content-Type', 'application/json')
        .send('{ invalid json }')
        .expect(400);

      expect(response.body.error).toContain('Invalid JSON');
    });
  });
});
```

## Usage Instructions

### 1. Copy Template
Choose the appropriate template for your test case.

### 2. Replace Placeholders
Replace all placeholders in brackets with actual values:
- `[SERVICE_NAME]` → `TradingService`
- `[METHOD_NAME]` → `executeTrade`
- `[ENDPOINT_PATH]` → `/api/trades`
- etc.

### 3. Customize Test Data
Update test data generation to match your domain:
```typescript
const inputData = testDataFactory.generateTradeRequest();
const expectedResult = testDataFactory.generateTradeExecution();
```

### 4. Add Domain-Specific Assertions
Include assertions specific to your business logic:
```typescript
expect(result.executionPrice).toBeCloseTo(marketPrice, 2);
expect(result.fees).toBeLessThanOrEqual(result.quantity * result.price * 0.001);
```

### 5. Configure Test Environment
Ensure test infrastructure matches your requirements:
- Database schemas
- Mock service configurations  
- Authentication setup
- Performance thresholds

This template library provides a solid foundation for comprehensive testing while maintaining consistency across the Neural Trader V2 test suite.