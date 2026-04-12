INSERT INTO taxpayers (tin, name, address) VALUES
    ('100011', 'Test Supplier Company', 'Test Address 1'),
    ('100021', 'Test Customer Company', 'Test Address 2')
ON CONFLICT (tin) DO NOTHING;
