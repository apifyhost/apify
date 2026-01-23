import { Card, Empty } from 'antd';
import { DatabaseOutlined } from '@ant-design/icons';

export const SchemasPage = () => {
  return (
    <div>
      <h1 style={{ marginBottom: 24 }}>表结构管理</h1>
      <Card>
        <Empty
          image={<DatabaseOutlined style={{ fontSize: 64, color: '#d9d9d9' }} />}
          description={
            <div>
              <p>表结构管理功能开发中...</p>
              <p>将支持查看和管理数据库表结构</p>
            </div>
          }
        />
      </Card>
    </div>
  );
};
