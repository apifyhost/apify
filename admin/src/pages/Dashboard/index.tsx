import { Card, Col, Row, Statistic, Alert, Typography, Space, Button, List } from 'antd';
import {
  ApiOutlined,
  DatabaseOutlined,
  CloudServerOutlined,
  ArrowUpOutlined,
  RocketOutlined,
  FileTextOutlined,
  GithubOutlined,
} from '@ant-design/icons';
import { useGetApisQuery, useGetDataSourcesQuery, useGetListenersQuery } from '@/services/api';

const { Title, Paragraph, Text } = Typography;

export const Dashboard = () => {
  const { data: apis = [], isLoading: apisLoading } = useGetApisQuery();
  const { data: dataSources = [], isLoading: dsLoading } = useGetDataSourcesQuery();
  const { data: listeners = [], isLoading: listenersLoading } = useGetListenersQuery();

  const quickStartGuide = [
    {
      title: '1. 配置数据源',
      description: '首先在"数据源"页面添加 PostgreSQL 或 SQLite 数据库连接',
      icon: <DatabaseOutlined />,
      link: '/datasources',
    },
    {
      title: '2. 创建监听器',
      description: '在"监听器"页面配置 HTTP 服务监听端口和路径',
      icon: <CloudServerOutlined />,
      link: '/listeners',
    },
    {
      title: '3. 定义 API',
      description: '在"API 配置"页面设置 RESTful API 端点和数据表映射',
      icon: <ApiOutlined />,
      link: '/apis',
    },
  ];

  return (
    <div>
      <div style={{ marginBottom: 24 }}>
        <Title level={2}>欢迎使用 Apify Admin</Title>
        <Paragraph type="secondary">
          零代码 API 生成平台 - 通过配置快速创建 RESTful API
        </Paragraph>
      </div>

      <Row gutter={[16, 16]} style={{ marginBottom: 24 }}>
        <Col xs={24} sm={12} lg={8}>
          <Card loading={apisLoading}>
            <Statistic
              title="API 配置"
              value={apis.length}
              prefix={<ApiOutlined />}
              valueStyle={{ color: '#3f8600' }}
              suffix={apis.length > 0 ? <ArrowUpOutlined /> : undefined}
            />
            <Button type="link" href="/admin/apis" style={{ padding: 0, marginTop: 8 }}>
              查看详情 →
            </Button>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <Card loading={dsLoading}>
            <Statistic
              title="数据源"
              value={dataSources.length}
              prefix={<DatabaseOutlined />}
              valueStyle={{ color: '#1890ff' }}
            />
            <Button type="link" href="/admin/datasources" style={{ padding: 0, marginTop: 8 }}>
              查看详情 →
            </Button>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <Card loading={listenersLoading}>
            <Statistic
              title="监听器"
              value={listeners.length}
              prefix={<CloudServerOutlined />}
              valueStyle={{ color: '#cf1322' }}
            />
            <Button type="link" href="/admin/listeners" style={{ padding: 0, marginTop: 8 }}>
              查看详情 →
            </Button>
          </Card>
        </Col>
      </Row>

      {apis.length === 0 && dataSources.length === 0 && (
        <Alert
          message="开始使用 Apify"
          description="您还没有配置任何 API，请按照下方的快速入门指南开始配置。"
          type="info"
          showIcon
          icon={<RocketOutlined />}
          style={{ marginBottom: 24 }}
        />
      )}

      <Row gutter={[16, 16]}>
        <Col xs={24} lg={16}>
          <Card title="快速入门指南" bordered={false}>
            <List
              dataSource={quickStartGuide}
              renderItem={(item) => (
                <List.Item>
                  <List.Item.Meta
                    avatar={
                      <div
                        style={{
                          width: 40,
                          height: 40,
                          borderRadius: '50%',
                          background: '#1890ff',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          color: '#fff',
                          fontSize: 20,
                        }}
                      >
                        {item.icon}
                      </div>
                    }
                    title={<Text strong>{item.title}</Text>}
                    description={item.description}
                  />
                  <Button type="link" href={`/admin${item.link}`}>
                    前往配置 →
                  </Button>
                </List.Item>
              )}
            />
          </Card>
        </Col>

        <Col xs={24} lg={8}>
          <Card title="相关资源" bordered={false}>
            <Space direction="vertical" style={{ width: '100%' }}>
              <Button
                type="link"
                icon={<FileTextOutlined />}
                href="https://github.com/apify/apify"
                target="_blank"
                block
                style={{ textAlign: 'left', padding: '8px 0' }}
              >
                查看文档
              </Button>
              <Button
                type="link"
                icon={<GithubOutlined />}
                href="https://github.com/apify/apify"
                target="_blank"
                block
                style={{ textAlign: 'left', padding: '8px 0' }}
              >
                GitHub 仓库
              </Button>
              <Button
                type="link"
                icon={<ApiOutlined />}
                href="/openapi"
                target="_blank"
                block
                style={{ textAlign: 'left', padding: '8px 0' }}
              >
                OpenAPI 文档
              </Button>
            </Space>
          </Card>

          <Card
            title="系统信息"
            bordered={false}
            style={{ marginTop: 16 }}
          >
            <Space direction="vertical" style={{ width: '100%' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <Text type="secondary">版本:</Text>
                <Text strong>0.1.0</Text>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <Text type="secondary">Control Plane:</Text>
                <Text strong>运行中</Text>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <Text type="secondary">端口:</Text>
                <Text strong>4000</Text>
              </div>
            </Space>
          </Card>
        </Col>
      </Row>
    </div>
  );
};
